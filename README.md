# Gordarg Android App

## To build and run app
```bash
./build.sh build && ./build.sh run
```

## To install Apache ANT
```bash
sudo apt instal ant
ant -v
# > Apache Ant(TM) version 1.10.12
```

## To generate _keystore_
```bash
keytool -genkey -v -keystore release.keystore -alias alias_name -keyalg RSA
```

## To install androidsdk
```bash
sudo snap install androidsdk
androidsdk --list
androidsdk "platform-tools"
androidsdk "platforms;android-28"
androidsdk "build-tools;34.0.0"
```

## To install android debug bridge
```bash
sudo apt install adb
```

# Learn more

<https://androidsdkmanager.azurewebsites.net/Buildtools>

<https://medium.com/@authmane512/how-to-do-android-development-faster-without-gradle-9046b8c1cf68>