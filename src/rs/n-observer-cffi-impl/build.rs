use std::{collections::HashMap, io::Result, path::Path};

use async_cffi_codegen::{generate_py, generate_rs};

fn main() -> Result<()> {
    let ior_trait_schema = n_observer::InnerObserverReceiver_schema();
    generate_rs(
        &ior_trait_schema,
        &get_rs_target_path("ior_cffi_traits.rs"),
        &HashMap::new(),
    )?;
    generate_py(
        &ior_trait_schema,
        &get_py_target_path("ior_cffi_traits.py"),
        &HashMap::new(),
    )?;

    let publisher_trait_schema = n_observer::Publisher_schema();
    generate_rs(
        &publisher_trait_schema,
        &get_rs_target_path("publisher_cffi_traits.rs"),
        &HashMap::new(),
    )?;
    generate_py(
        &publisher_trait_schema,
        &get_py_target_path("publisher_cffi_traits.py"),
        &HashMap::new(),
    )?;

    let observable_trait_schema = n_observer::Observable_schema();
    let ior_super_schema = n_observer::InnerObserverReceiver_schema();
    let publisher_super_schema = n_observer::Publisher_schema();
    let observable_supertraits = HashMap::from([
        (ior_super_schema.name.clone(), ior_super_schema),
        (publisher_super_schema.name.clone(), publisher_super_schema),
    ]);
    generate_rs(
        &observable_trait_schema,
        &get_rs_target_path("observable_cffi_traits.rs"),
        &observable_supertraits,
    )?;
    generate_py(
        &observable_trait_schema,
        &get_py_target_path("observable_cffi_traits.py"),
        &observable_supertraits,
    )?;

    Ok(())
}

fn get_rs_target_path(filename: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(filename)
}

fn get_py_target_path(filename: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("py")
        .join("centconf")
        .join("observer_cffi")
        .join(filename)
}
