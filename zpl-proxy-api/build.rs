fn main() {
    // embed_migrations! cannot itself track SQL changes for Cargo rebuilds.
    // https://docs.rs/diesel_migrations/2.3.2/diesel_migrations/macro.embed_migrations.html
    println!("cargo:rerun-if-changed=migrations");
}
