docker run ^
--volume %userprofile%\IdeaProjects\rs_weather_station:/home/cross/project ^
--volume %userprofile%\IdeaProjects\rs_weather_station\infrastructure\cross-deps:/home/cross/deb-deps ^
-e PKG_CONFIG_ALLOW_CROSS=1 ^
-e PKG_CONFIG_PATH=/home/cross/pi-tools/arm-bcm2708/arm-rpi-4.9.3-linux-gnueabihf/arm-linux-gnueabihf/sysroot/usr/lib/arm-linux-gnueabihf/pkgconfig/ ^
rust-nightly-pi-cross ^
build --release


ssh pi@192.168.0.29 "sudo systemctl stop rs_weather_station"
scp ../target/arm-unknown-linux-gnueabihf/release/rs_weather_station pi@192.168.0.29:/opt/rs_weather_station
ssh pi@192.168.0.29 "sudo systemctl start rs_weather_station"