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

`scripts/check-kernel-config.sh` сверяет `kernel/device-requirements.txt` с извлеченным config: 16 требований, из них 10 обязательных и 6 желательных. Результат на ядре pmOS v25.12: все обязательные выполнены (devtmpfs, proc, sysfs, tmpfs, UNIX, ELF, VT и framebuffer_console, ext4, evdev). Из конфига также известно, что `CONFIG_EXT4_FS=y`, `CONFIG_MMC_SUNXI=y`, `CONFIG_MMC_BLOCK=y` и `CONFIG_DRM_SUN4I=y` встроены в ядро, поэтому rootfs монтируется без initramfs, а `init=/sbin/liinit` работает напрямую. Одно предупреждение:

- `CONFIG_ANDROID_BINDERFS=n` при `CONFIG_ANDROID_BINDER_IPC=y`: binder доступен, но binderfs-монтирование включается нашим фрагментом конфигурации при пересборке ядра pmOS.

Модули в pmOS-ядре сжаты zstd (`.ko.zst`), а ядро собрано без `CONFIG_MODULE_DECOMPRESS`: `scripts/build-rootfs.sh` распаковывает оба дерева модулей (pmOS через `zstd`, virt через `gzip`) и переписывает `modules.dep`/`modules.alias` на распакованные пути, поэтому busybox modprobe работает с готовыми `.ko`.

## Rootfs

`scripts/build-rootfs.sh` собирает дистрибутивный rootfs (в CI в job `device`):

- база: alpine minirootfs aarch64 (busybox, musl, apk для расширения на устройстве);
- пакеты Afflictio: `liinit` как `/sbin/init`, `tiosessd`, `tioctl`, `tiocompd`, `binderctl`, `probe.sh`;
- пользователи: root и `mel0k1` (uid 1000, пароли залочены, автологин через tiosessd с реальным setuid);
- оба дерева модулей: `6.12.118-0-virt` для QEMU и `6.17.5` для PinePhone;
- `/etc/afflictio/init.toml` с секциями `[[modules]]`, `[[mounts]]` (binderfs) и службами tiosessd/tiocompd/probe;
- `/boot` с Image и тремя DTB, `/extlinux/extlinux.conf` с `root=UUID`;
- `/etc/afflictio/manifest.json` с версией и списком пакетов;
- `out/rootfs.ext4`: ext4 c фиксированным UUID, `/home/mel0k1` получает uid 1000 через `debugfs sif`.

Тот же rootfs загружается в QEMU в job `rootfs` (`scripts/qemu-rootfs-smoke.sh`): мини-initramfs (`scripts/mkrootfs-initramfs.sh`) поднимает `virtio_blk` и `ext4`, делает `switch_root` на rootfs.ext4, дальше PID 1 настоящий `liinit` из rootfs. Сценарий проходит с маркерами SMOKE_READY, SMOKE_SWITCH_VT1_OK, SMOKE_SWITCH_VT2_OK, SMOKE_FRAMES_DIFFER, SMOKE_UID_OK (проверка, что сессии работают от uid 1000), SMOKE_KMS_FLIPS_OK (композитор на virtio-gpu), SMOKE_RESPAWN_OK и корректным poweroff.

## Образ для SD-карты

`scripts/pack-device-bundle.sh` собирает бандл и три flashable-образа:

```
out/device/pinephone/
  bundle/
    boot/                     Image, DTB 1.1/1.2/1.2b, extlinux/extlinux.conf
    uboot/pine64-pinephone/   u-boot-sunxi-with-spl-{528,552,624}.bin
    rootfs/                   дерево rootfs (тот же, что в rootfs.ext4)
    rootfs.ext4
  afflictio-pinephone-528.img
  afflictio-pinephone-552.img
  afflictio-pinephone-624.img
```

Каждый образ: MBR с одним разделом ext4 (тип 0x83, LBA 2048), `u-boot-sunxi-with-spl` на смещении 8192 байт (сектор 16, стандарт sunxi), содержимое `rootfs.ext4` с начала раздела. Внутри раздела лежат и rootfs, и `/boot`, и `/extlinux/extlinux.conf`: u-boot читает ext4 напрямую, `fdt` в extlinux не указан, потому что каждый вариант u-boot передает собственный DTB, совпадающий с ревизией платы (1.1, 1.2, 1.2b).

Проверка образа (`scripts/assemble-img.py verify` + debugfs): подпись MBR, magic `eGON.BT0` на смещении 8192, ext4-superblock в начале раздела, бинари и passwd внутри раздела, `/boot/Image`, extlinux с `root=UUID`, манифест. mke2fs, debugfs и python работают без root, поэтому конвейер выполняется в CI.

## Прошивка SD-карты (рецепт для сессии с железом)

Не проверено на устройстве, выполняется вручную при bring-up:

```sh
# определить устройство SD-карты (последний вывод dmesg после вставки), затем:
sudo dd if=out/device/pinephone/afflictio-pinephone-624.img of=/dev/sdX bs=4M conv=fsync status=progress
sync
```

Вариант u-boot внутри образа определяет тайминги памяти; соответствие варианта ревизии платы уточняется при bring-up (в pmOS u-boot-pinephone вариант 624 используется для свежих ревизий). На загрузочной консоли (ttyS0, 115200 8N1, разъем UART на нижнем торце) виден вывод ядра и liinit; на экране PinePhone появляется композиторный кадр tiocompd поверх KMS Allwinner, а на tty1 и tty2 открываются shell-сессии пользователя mel0k1.

Быстрая перезагрузка в u-boot без SD: на плате держится `fastboot 0` в u-boot, поэтому при bring-up возможна прошивка eMMC через `fastboot flash` (gadget на USB-C, host-команда `fastboot devices` должна увидеть PinePhone). Для SD-загрузки fastboot не требуется.

Ожидаемые точки отладки при bring-up: модуль подсветки (pwm-backlight входит в DTB pmOS), кнопки и клавиатура через USB-C dock (evdev), соответствие `root=UUID` фактическому разделу (UUID фиксирован в build-rootfs.sh и extlinux.conf), поведение binderfs-монтирования (ожидается пропуск с записью в лог до пересборки ядра pmOS).

## Что дальше

Этап 5 собран: rootfs с пользователями и манифестом, flashable-образы с проверкой, загрузка rootfs в QEMU. Оставшаяся часть этапа: прошивка на железо и тумблинг списка `[[modules]]` под реальный порядок устройств PinePhone при bring-up.
