# Deploying to crates.io

This document provides instructions on how to publish the `netmap-rs` crate to [crates.io](https://crates.io/).

## Prerequisites

- You have an account on [crates.io](https://crates.io/).
- You have `cargo` installed.

## Instructions

1. **Log in to crates.io:**

   Before you can publish the crate, you need to log in to your crates.io account using `cargo`:

   ```bash
   cargo login
   ```

   You will be prompted to enter your API token, which you can find on your [crates.io account page](https://crates.io/me).

2. **Publish the crate:**

   Once you are logged in, you can publish the crate by running the following command from the root of the project directory:

   ```bash
   cargo publish
   ```

   This will build the crate and upload it to crates.io.

   **Note:** Make sure you have incremented the version number in `Cargo.toml` before publishing a new version.
