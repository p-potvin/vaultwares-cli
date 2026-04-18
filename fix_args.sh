#!/bin/bash
sed -i 's/mod args;/pub mod args;/g' crates/vaultwares-cli/src/main.rs
sed -i 's/pub use app::\*/pub use app::\*\;\npub use args::\*/g' crates/vaultwares-cli/src/main.rs
