RUSTFLAGS='--cfg getrandom_backend="wasm_js"' cargo build --target wasm32-unknown-unknown
echo "done building"
wasm-bindgen --no-typescript --target web --out-dir ./out/ --out-name "naval_sketch" ./target/wasm32-unknown-unknown/debug/naval_sketch.wasm
echo "done generating"
