ssh pi@192.168.0.29 "sudo systemctl stop rs_weather_station"
scp ../app_config.toml pi@192.168.0.29:/opt/rs_weather_station
scp ../target/arm-unknown-linux-gnueabihf/release/rs_weather_station pi@192.168.0.29:/opt/rs_weather_station
ssh pi@192.168.0.29 "sudo systemctl start rs_weather_station"