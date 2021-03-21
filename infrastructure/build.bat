docker run ^
--volume %userprofile%\IdeaProjects\rs_weather_station:/home/cross/project ^
--volume %userprofile%\IdeaProjects\rs_weather_station\infrastructure\cross-deps:/home/cross/deb-deps ^
--volume %userprofile%\.cargo\registry:/home/cross/.cargo/registry ^
-e PKG_CONFIG_ALLOW_CROSS=1 ^
-e PKG_CONFIG_PATH=/home/cross/pi-tools/arm-bcm2708/arm-rpi-4.9.3-linux-gnueabihf/arm-linux-gnueabihf/sysroot/usr/lib/arm-linux-gnueabihf/pkgconfig/ ^
rust-nightly-pi-cross ^
build --release
