# Roomie Remote
Roomie Remote is a flexible remote control software capable of connecting to various devices. They support customizable presets which can be configured to work with `minidspd`'s HTTP API.

These settings were contributed by Vince_B (avsforum) and add buttons to interface with the device's master controls.

### plist file
```xml
<dict>
<key>brand</key>
<string>mrene</string>
<key>cat</key>
<string>minidsp-rs</string>
<key>codes</key>
<dict>
<key>CONFIG 1</key>
<string>POST /devices/0/config
Content-Type: application/json; charset=utf-8.

{"master_status":{"preset":0}}</string>
<key>CONFIG 2</key>
<string>POST /devices/0/config
Content-Type: application/json; charset=utf-8.

{"master_status":{"preset": 1}}</string>
<key>CONFIG 3</key>
<string>POST /devices/0/config
Content-Type: application/json; charset=utf-8.

{"master_status":{"preset": 2}}</string>
<key>CONFIG 4</key>
<string>POST /devices/0/config
Content-Type: application/json; charset=utf-8.

{"master_status":{"preset": 3}}</string>
<key>SOURCE ANALOG</key>
<string>POST /devices/0/config
Content-Type: application/json; charset=utf-8.

{"master_status":{"source": "Analog"}}</string>
<key>SOURCE TOSLINK</key>
<string>POST /devices/0/config
Content-Type: application/json; charset=utf-8.

{"master_status":{"source": "Toslink"}}</string>
<key>SOURCE USB</key>
<string>POST /devices/0/config
Content-Type: application/json; charset=utf-8.

{"master_status":{"source": "Usb"}}</string>
<key>MUTE ON</key>
<string>POST /devices/0/config
Content-Type: application/json; charset=utf-8.

{"master_status":{"mute": true}}</string>
<key>MUTE OFF</key>
<string>POST /devices/0/config
Content-Type: application/json; charset=utf-8.

{"master_status":{"mute": false}}</string>
</dict>
<key>method</key>
<string>http</string>
<key>type</key>
<integer>1</integer>
</dict>
```