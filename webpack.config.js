const WasmPackPlugin = require("@wasm-tool/wasm-pack-plugin");

module.exports = {
  target: "webworker",
  entry: "./worker.js",
  mode: "production",
  plugins: [
    new WasmPackPlugin({
      crateDirectory: __dirname,
    })
  ]
};