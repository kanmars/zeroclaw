编译命令

# 1) 装 cargo-zigbuild
cargo install --locked cargo-zigbuild

# 2) 装 zig（cargo-zigbuild 调它当 linker，必须有）
brew install zig            # macOS 推荐
# 或
pip3.11 install ziglang         # 跨平台后备

# 3) 验证
cargo zigbuild --version    # 应输出版本号
zig version                 # 应输出 0.13.x / 0.14.x
which cargo-zigbuild        # 应在 ~/.cargo/bin/


安装 交叉编译链
rustup target add x86_64-unknown-linux-musl

cargo zigbuild --release --locked --target x86_64-unknown-linux-musl
