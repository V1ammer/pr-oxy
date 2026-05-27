# pr-oxy

Ultra-low-latency forward HTTP/HTTPS proxy на чистом Rust.

- **HTTP** — чтение `Host`-заголовка, форвардинг на `:80`
- **HTTPS** — поддержка метода `CONNECT`, туннелирование TLS
- **Zero-copy** — `tokio::io::copy_bidirectional` без промежуточных буферов
- **Минимум кода** — ~90 строк, только необходимые зависимости

## Зависимости

- [Rust](https://rustup.rs/) (stable)
- (Опционально) [Nix](https://nixos.org/download/) + [direnv](https://direnv.net/) для изолированного окружения

## Развертывание на Ubuntu

### Способ 1: Через Nix + direnv (рекомендуется)

```bash
# Установка Nix
sh <(curl -L https://nixos.org/nix/install) --daemon

# Включение flakes
mkdir -p ~/.config/nix
echo "experimental-features = nix-command flakes" >> ~/.config/nix/nix.conf

# Установка direnv
sudo apt update && sudo apt install -y direnv
# Добавь хук в shell:
echo 'eval "$(direnv hook bash)"' >> ~/.bashrc

# Клонирование и вход в директорию
cd pr-oxy
direnv allow        # подтянет Rust toolchain автоматически

# Сборка и запуск
cargo build --release
./target/release/pr-oxy
```

### Способ 2: Без Nix (чистый Rust)

```bash
# Установка Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# В системе должен быть доступен C-linker (gcc/clang)
sudo apt update && sudo apt install -y build-essential

# Сборка и запуск
cargo build --release
./target/release/pr-oxy
```

## Конфигурация

Файл `proxy.toml` в рабочей директории:

```toml
bind = "0.0.0.0"
port = 8080
```

## Запуск

```bash
RUST_LOG=info ./target/release/pr-oxy
```

## Проверка

```bash
# HTTP
curl -x http://127.0.0.1:8080 -I http://example.com

# HTTPS (CONNECT)
curl -x http://127.0.0.1:8080 -I https://example.com
```

## Оптимизации release

```toml
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
```

- `lto = "fat"` — межмодульное слияние для удаления неиспользуемого кода
- `codegen-units = 1` — единый codegen-модуль = лучшее планирование
- `panic = "abort"` — отказ от раскрутки стека = меньше бинарник и быстрее
