const CopyWebpackPlugin = require("copy-webpack-plugin");
const path = require("path");
const crypto = require("crypto");
const crypto_orig_createHash = crypto.createHash;
crypto.createHash = (algorithm) =>
  crypto_orig_createHash(algorithm == "md4" ? "sha256" : algorithm);

module.exports = {
  entry: "./bootstrap.js",
  output: {
    path: path.resolve(__dirname, "..", "..", "docs"),
    filename: "bootstrap.js",
  },
  mode: "development",
  experiments: { asyncWebAssembly: true },
  plugins: [
    new CopyWebpackPlugin([
      "index.html",
      "AND-Regular.ttf",
      "../../templates/style.css",
    ]),
  ],
};
