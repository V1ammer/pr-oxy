# pr-oxy

Ultra-low-latency forward HTTP/HTTPS proxy на чистом Rust с Basic Auth.

- **HTTP** — чтение `Host`-заголовка, форвардинг на `:80`
- **HTTPS** — поддержка метода `CONNECT`, туннелирование TLS
- **Basic Auth** — `Proxy-Authorization: Basic base64(user:pass)`
- **Zero-copy** — `tokio::io::copy_bidirectional` без промежуточных буферов
- **Минимум кода** — ~90 строк, только необходимые зависимости

## Переменные окружения

| Переменная | Описание | По умолчанию |
|---|---|---|
| `PORT` | Порт прокси | `8080` |
| `USER` | Логин для Basic Auth | — |
| `PASS` | Пароль для Basic Auth | — |

Если `USER` и `PASS` не заданы — авторизация отключена.

## Быстрая установка на сервер (одной командой)

```bash
curl -fsSL https://raw.githubusercontent.com/killua/pr-oxy/master/deploy/install.sh | sudo bash -s -- 8080 admin secret
```

> **Важно:** замени `killua/pr-oxy` внутри `deploy/install.sh` на свой `username/repo` перед первым релизом.

Скрипт автоматически:
- Создаст пользователя `pr-oxy`
- Скачает последний бинарник из GitHub Releases
- Создаст `/opt/pr-oxy/.env` с переменными окружения
- Установит и запустит systemd unit с auto-restart

Управление после установки:
```bash
sudo systemctl status pr-oxy
sudo systemctl restart pr-oxy
sudo journalctl -u pr-oxy -f
```

## Разработка

### Способ 1: Через Nix + direnv

```bash
sh <(curl -L https://nixos.org/nix/install) --daemon
mkdir -p ~/.config/nix && echo "experimental-features = nix-command flakes" >> ~/.config/nix/nix.conf
sudo apt install -y direnv
echo 'eval "$(direnv hook bash)"' >> ~/.bashrc

cd pr-oxy
direnv allow
cargo build --release
PORT=8080 USER=admin PASS=secret ./target/release/pr-oxy
```

### Способ 2: Без Nix (чистый Rust)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
sudo apt update && sudo apt install -y build-essential

cargo build --release
PORT=8080 USER=admin PASS=secret ./target/release/pr-oxy
```

## CI/CD

- **CI** — проверка сборки на каждый PR/push
- **Release** — автоматическая сборка и публикация бинарника в GitHub Releases при пуше тега `v*`

```bash
git tag v0.1.0
git push origin v0.1.0
```

## Проверка

```bash
# Без авторизации (если USER/PASS не заданы)
curl -x http://127.0.0.1:8080 -I http://example.com

# С авторизацией
curl -x http://admin:secret@127.0.0.1:8080 -I http://example.com
curl -x http://admin:secret@127.0.0.1:8080 -I https://example.com
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
