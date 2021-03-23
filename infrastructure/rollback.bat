ssh pi@192.168.0.29 "sudo systemctl stop rs_weather_station"
ssh pi@192.168.0.29 "sudo cp /opt/rs_weather_station/app_config_bak.toml /opt/rs_weather_station/app_config.toml"
ssh pi@192.168.0.29 "sudo cp /opt/rs_weather_station/rs_weather_station_bak /opt/rs_weather_station/rs_weather_station"
ssh pi@192.168.0.29 "sudo systemctl start rs_weather_station"