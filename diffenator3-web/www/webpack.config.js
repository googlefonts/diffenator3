const CopyWebpackPlugin = require("copy-webpack-plugin");
const path = require("path");
const crypto = require("crypto");
const crypto_orig_createHash = crypto.createHash;
crypto.createHash = (algorithm) =>
  crypto_orig_createHash(algorithm == "md4" ? "sha256" : algorithm);

module.exports = {
  output: {
    path: path.resolve(__dirname, "..", "..", "docs"),
  },
  entry: {
    bootstrap: "./bootstrap.js",
    static: {
      import: "./ts/d3web-static.ts",
      filename: "../templates/script.js",
    },
  },
  mode: "development",
  devtool: "source-map",
  experiments: { asyncWebAssembly: true },
  plugins: [
    new CopyWebpackPlugin([
      "index.html",
      "AND-Regular.ttf",
      "../../templates/style.css",
    ]),
  ],
  module: {
    rules: [
      {
        test: /\.tsx?$/,
        use: "ts-loader",
        exclude: /node_modules/,
      },
    ],
  },
  resolve: {
    extensions: [".ts", ".js"],
  },
};
