# Boltrunner

Браузерная PvP-игра на Rust и Bevy с авторитетным игровым сервером и WebSocket-синхронизацией.

- `crates/shared` — протокол, движение и общие игровые типы.
- `crates/server` — серверная симуляция, регистрация попаданий и состояние матча.
- `crates/client` — соединение с сервером и обмен игровым состоянием.
- `crates/game` — игровая графика и интерфейс Bevy.
- `course/profile-api` — модуль профиля игрока: CRUD, JWT, тесты, JSON-логи и OpenAPI.

[Инструкция запуска API профиля, использования Swagger и сетевого взаимодействия](course/profile-api/README.md).

Для API профиля: `cargo run --manifest-path course/profile-api/Cargo.toml` с заданным `BOLTRUNNER_JWT_SECRET`. Игровая сборка и развёртывание описаны в `scripts/build-web.sh`, `.github/actions/build-preview/action.yml` и `.github/workflows`.
