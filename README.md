# v2raySYBAU

Безопасный сетевой слой для клиента профилей прокси. Это модуль ядра, который UI может подключить к Tauri/Electron или другому приложению.

## Возможности

- Пакетная проверка всех профилей (`ProfileTester.testAll`): TCP, ICMP и HTTP GET через адаптер прокси. Результаты немедленно публикуются в `TelemetryStore`: средняя задержка, процент потерь и скорость в B/s.
- ICMP в Windows не запускает `ping`: возвращается понятный результат `skipped`, поскольку ICMP часто требует привилегированного helper-а.
- Анализ пакетов выключен по умолчанию. `PacketCapture` сохраняет только метаданные, ограничивает как число записей, так и суммарный объём, и не пишет тела пакетов или секреты.
- В `normalizeSettings` есть DNS из ключа, локальный DNS, FakeDNS, порт локального DNS и пользовательский DoH-сервер; также включаются mux и фрагментация.
- Правила маршрутизации доменов (`proxy` / `direct` / `block`) и режим «прокси только для выбранных приложений» проверяются до передачи в ядро.
- `GitHubUpdater` проверяет GitHub Releases, уведомляет UI, перед запуском инсталлятора обязательно сверяет SHA-256 и проверяет подпись переданным в приложение verifier-ом. `updateSubscription` применяет ту же проверку для GitHub-артефакта подписки и требует callback для атомарной замены файла.

## Быстрый старт

```js
import { TelemetryStore } from './src/network/telemetry.js';
import { PacketCapture } from './src/network/capture.js';
import { ProfileTester } from './src/network/tester.js';

const telemetry = new TelemetryStore();
telemetry.on('sample', (sample) => renderLiveNodeMetrics(sample));
const tester = new ProfileTester({ telemetry, capture: new PacketCapture({ enabled: true }) });
await tester.testAll(profiles, { icmp: true, httpUrl: 'https://example.com/' });
```

`httpProxyProbe` принимает `fetchImpl`, который обязан действительно направлять запрос через `proxyUrl` (например, адаптер движка V2Ray). Встроенный Node `fetch` сам по себе прокси не применяет.

## Проверки

```bash
npm test
npm run check
```
