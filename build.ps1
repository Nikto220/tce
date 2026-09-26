$env:RUSTFLAGS="-C target-feature=+bmi2"; cargo build --release --target-dir target/pext
$env:RUSTFLAGS=""; cargo build --release --target-dir target/magics
