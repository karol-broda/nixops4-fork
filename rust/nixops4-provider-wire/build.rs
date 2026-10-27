use std::{env, fs, path::Path};

use typify::{TypeSpace, TypeSpaceSettings};

fn main() {
    for (schema_file, out_dir, out_file) in [
        (
            "resource-schema-v0.json",
            "generated/schema/resource",
            "v0.rs",
        ),
        (
            "resource-schema-v1.json",
            "generated/schema/resource",
            "v1.rs",
        ),
    ] {
        println!("cargo:rerun-if-changed={schema_file}");
        schema_to_rust(schema_file, out_dir, out_file);
    }
}

fn schema_to_rust(schema_file: &str, out_dir: &str, out_file: &str) {
    let schema = load_schema(schema_file);
    let type_space = build_type_space(&schema);
    emit(&type_space, out_dir, out_file);
}

fn load_schema(schema_file: &str) -> schemars::schema::RootSchema {
    let content = fs::read_to_string(schema_file).unwrap();
    serde_json::from_str(&content).unwrap()
}

fn build_type_space(schema: &schemars::schema::RootSchema) -> TypeSpace {
    let mut type_space = TypeSpace::new(
        TypeSpaceSettings::default()
            .with_derive("Debug".to_string())
            .with_derive("PartialEq".to_string())
            .with_derive("Clone".to_string())
            .with_crate(
                "json_patch",
                typify::CrateVers::parse("4.0.0").unwrap(),
                None,
            )
            .with_map_type("std::collections::BTreeMap".to_string()),
    );
    type_space
        .add_ref_types(schema.definitions.clone())
        .unwrap();
    type_space
}

fn emit(type_space: &TypeSpace, out_dir: &str, out_file: &str) {
    let contents =
        prettyplease::unparse(&syn::parse2::<syn::File>(type_space.to_stream()).unwrap());

    let mut out_path = Path::new(&env::var("OUT_DIR").unwrap()).to_path_buf();
    out_path.push(out_dir);
    fs::create_dir_all(out_path.clone()).unwrap();
    out_path.push(out_file);
    fs::write(out_path, contents).unwrap();
}
