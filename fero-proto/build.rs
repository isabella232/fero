// Copyright 2017 PingCAP, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// See the License for the specific language governing permissions and
// limitations under the License.

extern crate grpcio_compiler;
extern crate protobuf;

use std::fs::{self, File};
use std::env;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output};

use grpcio_compiler::codegen as grpc_gen;
use protobuf::codegen as pb_gen;
use protobuf::compiler_plugin::GenResult;
use protobuf::descriptor::{FileDescriptorProto, FileDescriptorSet};

fn run_command(cmd: &mut Command) -> Output {
    match cmd.output() {
        Err(e) => panic!("failed to run [{:?}]: {:?}", cmd, e),
        Ok(output) => output,
    }
}

/// Descriptor file to module file.
fn desc_to_module<P, G, W>(descriptor: P, output: P, mut gen: G, mut module: W)
where
    P: AsRef<Path>,
    G: FnMut(&[FileDescriptorProto], &[String]) -> Vec<GenResult>,
    W: Write,
{
    let proto_set: FileDescriptorSet = {
        let mut f = File::open(descriptor).unwrap();
        protobuf::parse_from_reader(&mut f).unwrap()
    };
    let files: Vec<_> = proto_set
        .get_file()
        .into_iter()
        .map(|f| f.get_name().to_owned())
        .collect();
    // All files need to be generated in our case.
    let results = gen(proto_set.get_file(), &files);
    let output_dir = output.as_ref();
    if !output_dir.exists() {
        fs::create_dir_all(output_dir).unwrap();
    }

    for res in results {
        let out_file = output_dir.join(&res.name);
        let mut f = File::create(&out_file).unwrap();
        f.write_all(&res.content).unwrap();
        let (module_name, _) = res.name.split_at(res.name.len() - 3); // ".rs".len() == 3
        writeln!(module, "pub mod {};", module_name).unwrap();
    }
}

/// Compile all related proto file to `FileDescriptorSet` and use it to generate
/// rust source.
///
/// Using `FileDescriptorSet` here so we don't need to compile the binaries like
/// protoc-gen-rust and grpc_rust_plugin.
fn compile(protos: &[&str], module: &str) {
    let out_dir = env::var("OUT_DIR").unwrap();
    let module_path = Path::new(&out_dir).join(module);
    if !module_path.exists() {
        fs::create_dir_all(&module_path).unwrap();
    }

    let mut protoc = Command::new("protoc");
    let mut desc_path = module_path.to_path_buf();
    desc_path.set_extension("desc");
    protoc.args(&["-o", &format!("{}", desc_path.display())]);
    protoc.args(&["-I/usr/include/", "-I."]);
    for proto in protos {
        protoc.arg(format!("{}", proto));
    }
    println!("Running: {:?}", protoc);
    let status = protoc.status().unwrap();
    if !status.success() {
        panic!("failed to run {:?}: {}", protoc, status);
    }

    let mod_rs = module_path.join("mod.rs");
    let mut module = File::create(mod_rs).unwrap();
    desc_to_module(&desc_path, &module_path, pb_gen::gen, &mut module);
    desc_to_module(&desc_path, &module_path, grpc_gen::gen, &mut module);
}

fn check_protoc() {
    let output = run_command(Command::new("protoc").arg("--version"));
    if !output.status.success() {
        panic!("protoc is required.");
    }
    let version = String::from_utf8(output.stdout).unwrap();
    let mut iter = version.split_whitespace();
    iter.next().unwrap();
    let vercode = iter.next().unwrap();
    let marjor: usize = vercode.split('.').next().unwrap().parse().unwrap();
    if marjor < 3 {
        panic!("expect protoc 3.0.0+ is required, find {}", vercode);
    }
}

fn main() {
    check_protoc();

    compile(&["src/fero.proto", "google/protobuf/timestamp.proto"], "fero")
}

