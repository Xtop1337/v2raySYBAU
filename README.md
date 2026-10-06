# V2Ray Sybau — Windows VPN Client

Современный клиент для V2Ray/Xray с поддержкой VLESS и VMess на Windows (WPF, .NET 8).

## 🚀 Функции

- **Профили**: Импорт/экспорт VLESS и VMess URI
- **Подписки**: Загрузка профилей из URL с автоматическим обновлением
- **Прокси**: Системный SOCKS прокси на localhost:10808
- **Автозапуск**: Запуск с системой и автоподключение к последнему профилю
- **Лог**: Журнал событий и ошибок в реальном времени
- **Группы**: Фильтрация профилей по группам
- **Темы**: Поддержка System/Light/Dark тем

## 📋 Требования

- **Windows 10+** (.NET 8 Desktop Runtime)
- **Xray/V2Ray core** — положите `xray.exe` в папку приложения или в подпапку `core/`

## 🔧 Ус��ановка и запуск

### Вариант 1: Из исходников

```bash
git clone https://github.com/Xtop1337/v2raySYBAU.git
cd v2raySYBAU
dotnet build V2RaySybau.sln
dotnet run --project src/V2RaySybau/V2RaySybau.csproj
```

### Вариант 2: Release сборка

```bash
dotnet publish -c Release -r win-x64 --self-contained false -o bin/Release/
```

Готовый exe будет в `bin/Release/V2RaySybau.exe`

### Вариант 3: Скачать готовый exe

[Скачать последний релиз](https://github.com/Xtop1337/v2raySYBAU/releases)

## 📝 Первый запуск

1. Скопируйте `xray.exe` (из [xray-core](https://github.com/XTLS/Xray-core/releases)) в папку приложения
2. Откройте приложение
3. Нажмите **Импорт** и вставьте:
   - `vless://...` или `vmess://...` ссылку
   - или URL подписки
4. Выберите профиль и нажмите **Подключить**
5. Трафик будет проходить через прокси на `127.0.0.1:10808`

## 🔌 Системный прокси

Отметьте "Системный прокси" чтобы автоматически переправлять весь трафик браузера через V2Ray.

## 🚀 Автозапуск

Включите "Запуск с системой" и отметьте "Автоподключение" чтобы приложение:
- Запускалось при входе в Windows
- Автоматически подключалось к последнему использованному профилю

## 🏗️ Архитектура

```
src/V2RaySybau/
├── UI/                          # WPF интерфейс
│   ├── MainWindow.xaml         # Главное окно
│   └── MainWindow.xaml.cs      # Логика подключения
├── Core/                        # Ядро V2Ray
│   ├── CoreProcessService.cs   # Запуск/остановка xray
│   └── XrayConfigAdapter.cs    # Генерация конфига
├── Models/                      # Модели данных
│   └── ConnectionProfile.cs    # Профиль подключения
├── Profiles/                    # Парсинг профилей
│   └── ProfileImportService.cs # Импорт VLESS/VMess
├── Settings/                    # Настройки приложения
│   ├── AppSettings.cs          # Класс настроек
│   └── SettingsService.cs      # Сохранение/загрузка
├── Storage/                     # Хранилище профилей
│   └── ProfileStore.cs         # JSON хранилище
└── Infrastructure/              # Windows интеграция
    └── WindowsIntegrationService.cs # Прокси и автозапуск
```

## 📦 Техстек

- **Framework**: .NET 8.0 (Windows)
- **UI**: WPF (XAML + C#)
- **JSON**: System.Text.Json
- **Сеть**: HttpClient, System.Diagnostics
- **Windows API**: Registry, WinINET

## 🐛 Известные ограничения

- Требует .NET 8 Desktop Runtime на целевой машине
- Поддерживает только VLESS и VMess (Shadowsocks, Trojan требуют расширения)
- Системный прокси работает только на уровне браузера (не на уровне системы)
- Xray/V2Ray core должен быть загружен отдельно

## 📄 Лицензия

MIT License — см. [LICENSE](LICENSE)

## 🤝 Contributing

Приветствуются Pull Request'ы! Для больших изменений сначала откройте Issue.

---

**Использование на свой риск**. Убедитесь, что вы имеете право использовать V2Ray/Xray в вашей стране/регионе.
