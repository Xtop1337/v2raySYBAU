# V2Ray Sybau Release Guide

## 🚀 Быстрая сборка EXE на Windows

### Вариант 1: Автоматическая сборка (рекомендуется)

```bash
build-release.bat
```

Скрипт автоматически:
1. Очистит предыдущие сборки
2. Восстановит NuGet пакеты
3. Соберет Release конфигурацию
4. Опубликует готовый EXE

**Результат**: `bin/Release/publish/V2RaySybau.exe`

### Вариант 2: Ручная сборка

```bash
# Восстановить пакеты
dotnet restore V2RaySybau.sln

# Собрать Release
dotnet build V2RaySybau.sln -c Release

# Опубликовать
dotnet publish src/V2RaySybau/V2RaySybau.csproj -c Release -r win-x64 --self-contained false -o bin/Release/publish
```

## 📦 Подготовка к Release

### 1. Скачать Xray Core

Перейти на https://github.com/XTLS/Xray-core/releases и скачать:
- `Xray-windows-64.zip` (для x64)

Распаковать `xray.exe` в папку с приложением:
```
bin/Release/publish/
├── V2RaySybau.exe
├── xray.exe         ← положить сюда
└── [другие файлы]
```

### 2. Проверить работоспособность

```bash
bin/Release/publish/V2RaySybau.exe
```

Приложение должно запуститься с главным окном.

### 3. Создать Release архив

```bash
# Создать архив со всеми необходимыми файлами
tar -czf V2RaySybau-1.0.0-win64.tar.gz -C bin/Release/publish .
```

Или на Windows:
```bash
# Использовать 7-Zip или встроенное сжатие
# Выбрать папку bin\Release\publish → Отправить → Сжатая папка (ZIP)
```

## 📝 Создание GitHub Release

1. Перейти на страницу **Releases** репозитория
2. Нажать **Create a new release**
3. Заполнить:
   - **Tag version**: `v1.0.0`
   - **Release title**: `V2Ray Sybau v1.0.0 - Windows VPN Client`
   - **Description**:
     ```markdown
     # V2Ray Sybau v1.0.0

     Windows VPN Client for VLESS/VMess with Xray/V2Ray core

     ## Features
     - VLESS и VMess профили
     - Импорт ссылок и подписок
     - Системный SOCKS прокси
     - Автозапуск и автоподключение
     - Журнал событий

     ## Installation
     1. Download `V2RaySybau-1.0.0-win64.zip`
     2. Extract to any folder
     3. Download xray.exe and place in the same folder
     4. Run `V2RaySybau.exe`

     ## Requirements
     - Windows 10+
     - .NET 8 Desktop Runtime
     ```

4. Нажать **Choose files** и загрузить:
   - `V2RaySybau-1.0.0-win64.zip`
   - или папку `bin/Release/publish/`

5. Нажать **Publish release**

## ✅ Проверка перед релизом

- [ ] EXE собирается без ошибок
- [ ] Приложение запускается
- [ ] Импорт профилей работает
- [ ] Подключение к xray работает (если xray.exe присутствует)
- [ ] Система прокси переключается
- [ ] Автозапуск регистрируется в реестре
- [ ] Логи записываются корректно
- [ ] Settings сохраняются в `%LOCALAPPDATA%\V2RaySybau`

## 🔗 Ссылки

- [.NET 8 Runtime Download](https://dotnet.microsoft.com/download/dotnet/8.0)
- [Xray-core Releases](https://github.com/XTLS/Xray-core/releases)
- [GitHub Releases Guide](https://docs.github.com/en/repositories/releasing-projects-on-github/managing-releases-in-a-repository)

---

**Примечание**: Приложение требует наличия `xray.exe` в одной папке. Без него импорт и настройки работают, но подключение невозможно.
