# PinePhone: первый target Afflictio

PinePhone (PINE64, Allwinner A64) выбран как первое устройство для этапа 3 дорожной карты. Аргументы: mainline-ядро с широкой поддержкой в postmarketOS (пакет `linux-postmarketos-allwinner`, 431 модуль), загрузка через mainline U-Boot, бут с microSD без прошивки внутренней памяти, дешевое и распространенное дев-устройство. Артефактный конвейер и сборка дистрибутива проверяются в CI в job `device`; прошивка на железо остается за сессией bring-up.

## Артефакты из пайплайна postmarketOS

Скрипт `scripts/fetch-pm-kernel.sh` тянет готовые артефакты pmOS-пайплайна (pmbootstrap собирает те же apk-пакеты из aports), версии запинены, с fallback-резолвом через APKINDEX:

| Артефакт | Версия | Что внутри |
| --- | --- | --- |
| `linux-postmarketos-allwinner` | 6.17.5_git20251026-r2 | Image 24 МБ, DTB PinePhone 1.1/1.2/1.2b, 431 модуль |
| `u-boot-pinephone` | 2023.01-r4 | u-boot-sunxi-with-spl-{528,552,624}.bin (SD/eMMC) |

Конфигурация ядра не публикуется отдельным файлом, но pmOS-ядро собрано с `CONFIG_IKCONFIG`: скрипт находит маркер `IKCFG_ST` в Image, распаковывает встроенный gzip-поток и сохраняет точный config в `out/device/pinephone/kernel.config` (2322 опции). Так проверка идет против конфигурации именно собранного ядра, без запросов к GitLab pmOS, который закрыт anti-bot-защитой.

## Проверка совместимости

`scripts/check-kernel-config.sh` сверяет `kernel/device-requirements.txt` с извлеченным config: 19 требований, из них 13 обязательных и 6 желательных. Результат на ядре pmOS v25.12: все обязательные выполнены (devtmpfs, proc, sysfs, tmpfs, UNIX, ELF, VT и framebuffer_console, ext4, evdev, USB-serial-набор модема). Из конфига также известно, что `CONFIG_EXT4_FS=y`, `CONFIG_MMC_SUNXI=y`, `CONFIG_MMC_BLOCK=y` и `CONFIG_DRM_SUN4I=y` встроены в ядро, поэтому rootfs монтируется без initramfs, а `init=/sbin/liinit` работает напрямую. Единственное предупреждение прежнее:

- `CONFIG_ANDROID_BINDERFS=n` при `CONFIG_ANDROID_BINDER_IPC=y`: binder доступен, но binderfs-монтирование включается нашим фрагментом конфигурации при пересборке ядра pmOS.

Модули в pmOS-ядре сжаты zstd (`.ko.zst`), а ядро собрано без `CONFIG_MODULE_DECOMPRESS`: `scripts/build-rootfs.sh` распаковывает оба дерева модулей (pmOS через `zstd`, virt через `gzip`) и переписывает `modules.dep`/`modules.alias` на распакованные пути, поэтому busybox modprobe работает с готовыми `.ko`.

## Rootfs

`scripts/build-rootfs.sh` собирает дистрибутивный rootfs (в CI в job `device`):

- база: alpine minirootfs aarch64 (busybox, musl, apk для расширения на устройстве);
- пакеты Afflictio: `liinit` как `/sbin/init`, `tiosessd`, `tioctl`, `tiocompd`, `binderctl`, `lioatd`, `modemctl`, `liota`, `probe.sh`;
- пользователи: root и `mel0k1` (uid 1000, пароли залочены, автологин через tiosessd с реальным setuid);
- оба дерева модулей: `6.12.118-0-virt` для QEMU и `6.17.5` для PinePhone;
- `/etc/afflictio/init.toml` с секциями `[[modules]]`, `[[mounts]]` (binderfs) и службами lioatd/tiosessd/tiocompd/probe, `/etc/afflictio/lioat.toml` со списком модемных устройств, `/etc/afflictio/slots.toml` с картой A/B-слотов;
- `/boot` как точка монтирования boot-раздела;
- `/etc/afflictio/manifest.json` с версией и списком пакетов;
- `out/rootfs.ext4`: ext4 c фиксированным UUID слота a, `/home/mel0k1` получает uid 1000 через `debugfs sif`.

Тот же rootfs загружается в QEMU в job `rootfs` (`scripts/qemu-rootfs-smoke.sh`): образ собирается из boot-раздела (extlinux.conf с label a и b) и двух слотов, мини-initramfs (`scripts/mkrootfs-initramfs.sh`) монтирует boot-раздел, разрешает активный слот по `default` label и его `root=UUID=` через slots.toml, делает `switch_root` на выбранный слот, дальше PID 1 настоящий `liinit` из rootfs. Сценарий проходит с маркерами SMOKE_READY, SMOKE_SWITCH_VT1_OK, SMOKE_SWITCH_VT2_OK, SMOKE_FRAMES_DIFFER, SMOKE_UID_OK, SMOKE_KMS_FLIPS_OK, SMOKE_RESPAWN_OK, `SMOKE_MODEM_AT_OK` (модемная заглушка на втором PL011), `SMOKE_AB_STATUS_OK` и `SMOKE_AB_SWITCH_OK` на слоте a, затем машина загружается второй раз: initramfs читает уже переписанный extlinux.conf, монтирует слот b, liinit пишет boot-ok, и probe фиксирует `SMOKE_AB_BOOT_B_OK`.

## Модем

PinePhone несет Quectel EG25-G на USB; mainline-ядро pmOS собирает его драйверы модулями (`CONFIG_USB_OPTION`, `CONFIG_USB_NET_QMI_WWAN` входят в список обязательных требований как `=y|m`), и AT-порт модема появляется как `/dev/ttyUSB2`. Демон `lioatd` читает `/etc/afflictio/lioat.toml` и открывает первый доступный порт из списка `devices` — на железе это ttyUSB2, в QEMU второй PL011 `/dev/ttyAMA1`. Опрос раз в `probe_interval` секунд обновляет снимок: модель, состояние SIM (`READY`), уровень сигнала в CSQ и dBm, регистрацию на сети; `modemctl status` печатает его в JSON, `modemctl at "AT+..."` отправляет произвольную команду через unix-socket демона. В QEMU-смоук реальный EG25-G заменяет `scripts/modem-emu.py` — заглушка отвечает тем же набором команд, и lioatd фиксирует `SMOKE_MODEM_AT_OK ... pin=READY rssi=21`. Передача данных через QMI и проверка исчезновения USB-устройства остаются на bring-up.

## A/B-обновления

Образ стал трехразделным: p1 — boot (Image, DTB трех ревизий, extlinux.conf с label a и b), p2 — слот a (`aff1a001-...`), p3 — слот b (`aff1a002-...`); оба слота на заводе заполняются одним rootfs.ext4. Конфигурация `/etc/afflictio/slots.toml` хранит базовый диск (`/dev/mmcblk0`), номер boot-раздела и карту имя/раздел/UUID; пути строятся по правилу именования разделов ядра, поэтому тот же конфиг работает в QEMU с `/dev/vda`. Утилита `liota`:

- `liota status` — JSON-снимок: активный слот (по `root=UUID=` из cmdline), неактивный, наличие boot-ok маркера;
- `liota switch` — монтирует boot-раздел, переписывает `default` в extlinux.conf на неактивный слот (запись во временный файл + rename), отмонтирует;
- `liota install PAYLOAD` — побайтово пишет payload в раздел неактивного слота с fsync; после установки нужен явный `liota switch` и перезагрузка.

liinit пишет `/var/lib/afflictio/boot-ok` после запуска служб, так что слот подтверждает свою загрузку. В QEMU весь цикл проверен двумя загрузками: на слоте a `status` подтверждает карту слотов, `switch` переписывает extlinux.conf, вторая загрузка попадает на слот b и фиксирует `SMOKE_AB_BOOT_B_OK`. Откат — обратный `liota switch`. Пакет обновления подписан. Формат AFFUPD1: магия, JSON-манифест (версия, размер, sha256 образа), ed25519-подпись над байтами манифеста, образ. Публичный ключ лежит в rootfs как `/etc/afflictio/updates.pub` (переопределяется через AFFLICTIO_UPDATE_PUB), приватный остается вне устройства: `scripts/build-rootfs.sh` при сборке генерирует пару в `out/keys` и кладет публичный ключ в образ, `liota sign KEY IMAGE UPDATE` собирает подписанный пакет, `liota verify PAYLOAD` и `liota install PAYLOAD` проверяют подпись, размер и хеш потоково перед записью. В smoke цикл прогоняется на двух дополнительных virtio-дисках: подписанный пакет подтверждается (`SMOKE_AB_SIGN_OK`), испорченный — отвергается (`SMOKE_AB_SIGN_REJECT`). Доставка пакета по сети остается на bring-up.

## Образ для SD-карты

`scripts/pack-device-bundle.sh` собирает бандл и три flashable-образа:

```
out/device/pinephone/
  bundle/
    boot/                     Image, DTB 1.1/1.2/1.2b, extlinux/extlinux.conf (label a и b)
    uboot/pine64-pinephone/   u-boot-sunxi-with-spl-{528,552,624}.bin
    rootfs/                   дерево rootfs (тот же, что в rootfs.ext4)
    rootfs.ext4               содержимое обоих слотов
  afflictio-pinephone-528.img
  afflictio-pinephone-552.img
  afflictio-pinephone-624.img
```

Каждый образ: MBR с тремя разделами ext4 (тип 0x83), `u-boot-sunxi-with-spl` на смещении 8192 байт (сектор 16, стандарт sunxi); p1 — boot (Image, DTB, extlinux.conf, 96 МБ, LBA 2048), p2 — слот a, p3 — слот b (по 512 МБ, содержимое rootfs.ext4). `fdt` в extlinux не указан, потому что каждый вариант u-boot передает собственный DTB, совпадающий с ревизией платы (1.1, 1.2, 1.2b); u-boot находит boot-раздел через distro bootcmd и читает extlinux.conf с label a и b, `default` выбирает активный слот.

Проверка образа (`scripts/assemble-img.py verify` + debugfs): подпись MBR, три раздела с типом 0x83 и возрастающими началами, magic `eGON.BT0` на смещении 8192, ext4-суперблок в начале каждого раздела, Image и оба `root=UUID` в boot-разделе, `default a`, бинари (8) и passwd внутри слота, манифест и slots.toml с обоими UUID. mke2fs, debugfs и python работают без root, поэтому конвейер выполняется в CI.

## Прошивка SD-карты (рецепт для сессии с железом)

Не проверено на устройстве, выполняется вручную при bring-up:

```sh
# определить устройство SD-карты (последний вывод dmesg после вставки), затем:
sudo dd if=out/device/pinephone/afflictio-pinephone-624.img of=/dev/sdX bs=4M conv=fsync status=progress
sync
```

Вариант u-boot внутри образа определяет тайминги памяти; соответствие варианта ревизии платы уточняется при bring-up (в pmOS u-boot-pinephone вариант 624 используется для свежих ревизий). На загрузочной консоли (ttyS0, 115200 8N1, разъем UART на нижнем торце) виден вывод ядра и liinit; на экране PinePhone появляется композиторный кадр tiocompd поверх KMS Allwinner, а на tty1 и tty2 открываются shell-сессии пользователя mel0k1.

Быстрая перезагрузка в u-boot без SD: на плате держится `fastboot 0` в u-boot, поэтому при bring-up возможна прошивка eMMC через `fastboot flash` (gadget на USB-C, host-команда `fastboot devices` должна увидеть PinePhone). Для SD-загрузки fastboot не требуется.

Ожидаемые точки отладки при bring-up: модуль подсветки (pwm-backlight входит в DTB pmOS), кнопки и клавиатура через USB-C dock (evdev), соответствие `root=UUID` фактическому разделу (UUID фиксированы в build-rootfs.sh и slots.toml), поведение binderfs-монтирования (ожидается пропуск с записью в лог до пересборки ядра pmOS), порядок инициализации модема EG25-G за драйвером option.

## Что дальше

Этапы 5-7 собраны: rootfs с пользователями и манифестом, flashable-образы с проверкой, загрузка rootfs в QEMU, модемный AT-слой с заглушкой EG25-G, A/B-слоты с переключением и проверкой в двух загрузках. Оставшаяся часть: прошивка на железо, тумблинг списка `[[modules]]` под реальный порядок устройств PinePhone, встреча lioat с реальным EG25-G и первый live-цикл liota install/switch на железе при bring-up.