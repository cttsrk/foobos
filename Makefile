# Lines beginning with @ are silent, lines beginning #@ are silent comments

all:
	# Documentation
	cargo doc --workspace -Zrustdoc-map --document-private-items
	# Linting
	cargo clippy --target x86_64-unknown-uefi -- \
		-A clippy::print_with_newline \
		-A clippy::redundant_field_names \
		-F clippy::missing_docs_in_private_items
	# Builing
	cargo build --release
	@# cargo clippy --target .cargo/aarch64-unknown-uefi.json -- -A clippy::print_with_newline
	@# cargo clippy --target .cargo/riscv64-unknown-uefi.json -- -A clippy::print_with_newline
	@# cargo build --target x86_64-unknown-uefi
	@# cargo build --target .cargo/aarch64-unknown-uefi.json
	@# broken, llvm issues: cargo build --target .cargo/riscv64-unknown-uefi.json
	
clean:
	cargo clean
