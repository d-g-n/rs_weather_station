ssh pi@192.168.0.29 "sudo systemctl stop rs_weather_station"
ssh pi@192.168.0.29 "sudo cp /opt/rs_weather_station/app_config.toml /opt/rs_weather_station/app_config_bak.toml"
ssh pi@192.168.0.29 "sudo cp /opt/rs_weather_station/rs_weather_station /opt/rs_weather_station/rs_weather_station_bak"
scp ../app_config.toml pi@192.168.0.29:/opt/rs_weather_station
scp ../target/arm-unknown-linux-gnueabihf/release/rs_weather_station pi@192.168.0.29:/opt/rs_weather_station
ssh pi@192.168.0.29 "sudo systemctl start rs_weather_station"