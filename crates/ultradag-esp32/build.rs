// Build script for ESP-IDF-targeted Rust binaries. Delegates to `embuild`,
// which downloads/builds the ESP-IDF C toolchain components, configures
// kconfig from sdkconfig.defaults, and emits the linker arguments the
// `ldproxy` linker shim in .cargo/config.toml consumes.
//
// Without this, cargo cannot produce a flashable ELF — the Rust code
// compiles but there is nothing for it to link against.
fn main() {
    embuild::espidf::sysenv::output();
}
