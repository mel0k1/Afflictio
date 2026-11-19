# PinePhone: первый target Afflictio

PinePhone (PINE64, Allwinner A64) выбран как первое устройство для этапа 3 дорожной карты. Аргументы: mainline-ядро с широкой поддержкой в postmarketOS (пакет `linux-postmarketos-allwinner`, 431 модуль), загрузка через mainline U-Boot, бут с microSD без прошивки внутренней памяти, дешевое и распространенное дев-устройство. До этапа 5 (дистрибутив) образ не прошивается на железо, но весь артефактный конвейер уже проверяется в CI в job `device`.

## Артефакты из пайплайна postmarketOS

Скрипт `scripts/fetch-pm-kernel.sh` тянет готовые артефакты pmOS-пайплайна (pmbootstrap собирает те же apk-пакеты из aports), версии запинены, с fallback-резолвом через APKINDEX:

| Артефакт | Версия | Что внутри |
| --- | --- | --- |
| `linux-postmarketos-allwinner` | 6.17.5_git20251026-r2 | Image 24 МБ, DTB PinePhone 1.1/1.2/1.2b, 431 модуль |
| `u-boot-pinephone` | 2023.01-r4 | u-boot-sunxi-with-spl-{528,552,624}.bin (SD/eMMC) |

Конфигурация ядра не публикуется отдельным файлом, но pmOS-ядро собрано с `CONFIG_IKCONFIG`: скрипт находит маркер `IKCFG_ST` в Image, распаковывает встроенный gzip-поток и сохраняет точный config в `out/device/pinephone/kernel.config` (2322 опции). Так проверка идет против конфигурации именно собранного ядра, без запросов к GitLab pmOS, который закрыт anti-bot-защитой.

## Проверка совместимости

`scripts/check-kernel-config.sh` сверяет `kernel/device-requirements.txt` с извлеченным config: 16 требований, из них 10 обязательных и 6 желательных. Результат на ядре pmOS v25.12: все обязательные выполнены (devtmpfs, proc, sysfs, tmpfs, UNIX, ELF, **VT и framebuffer_console**, ext4, evdev), одно предупреждение:

- `CONFIG_ANDROID_BINDERFS=n` при `CONFIG_ANDROID_BINDER_IPC=y`: binder доступен, но binderfs-монтирование нужно будет включить в нашем фрагменте конфигурации при этапе 4 (libgbinder). Это тот самый случай из дорожной карты: фрагмент из kernel/ становится дополнением к defconfig pmOS, а не заменой.

Модули в pmOS-ядре сжаты zstd (`.ko.zst`): busybox modprobe их не распакует, на этапе bring-up железа понадобится kmod либо перекомпрессия набора в gzip. Отмечено как пункт списка bring-up.

## Бандл

`scripts/pack-device-bundle.sh` собирает `out/device/pinephone/bundle/`:

```
bundle/
  boot/
    Image                     ядро pmOS
    sun50i-a64-pinephone-1.2.dtb
    extlinux/extlinux.conf    cmdline: root=/dev/mmcblk0p2 init=/sbin/liinit console=tty1+ttyS0
  uboot/pine64-pinephone/     три варианта u-boot-sunxi-with-spl
  rootfs/                     каталог-прототип: busybox, liinit, tiosessd, tioctl, /etc/afflictio, /lib/modules/6.17.5
  rootfs.ext4                 тот же каталог, упакованный mke2fs -d (512 МБ ext4, label afflictio)
```

Целостность проверяется в скрипте через `debugfs`: бинари читаются из образа, session.toml с `[[autologin]]` на месте. mke2fs и debugfs работают без root, поэтому конвейер выполняется в CI.

## Загрузка (рецепт для сессии с железом)

Не проверено на устройстве, выполняется вручную при bring-up:

```sh
sfdisk /dev/sdX <<'EOF'
label: dos
start=1M, size=256M, type=83, bootable
start=257M, type=83
EOF
mkfs.ext4 -L boot /dev/sdX1
mkfs.ext4 -L afflictio /dev/sdX2
dd if=uboot/pine64-pinephone/u-boot-sunxi-with-spl-528.bin of=/dev/sdX bs=1024 seek=8 conv=notrunc
mount /dev/sdX1 /mnt/boot && cp -r bundle/boot/* /mnt/boot/
dd if=bundle/rootfs.ext4 of=/dev/sdX2 bs=4M
```

U-Boot с SD-карты грузит `extlinux/extlinux.conf` с первого раздела, ядро поднимает rootfs с `/sbin/liinit` как PID 1, liinit запускает tiosessd по `etc/init.toml` устройства, автологин-сессия появляется на fbcon-консоли через дисплей PinePhone (KMS-путь Allwinner). Ожидаемые точки отладки при bring-up: kmod для `.ko.zst`, порядок загрузки модулей (подсветка, кнопки, клавиатура через USB-C dock) и соответствие `/dev/mmcblk0p2` SD-карте (проверить по alias mmc0 в DTS).

## Что дальше

Этап 4 берет этот бандл как основу: включение binderfs в фрагменте, композитор поверх KMS-дисплея Allwinner, libgbinder для модема (eg25-manager в pmOS использует ровно этот путь). Этап 5 упаковывает бандл в полный образ с прошивкой через pmbootstrap.
