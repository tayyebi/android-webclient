#!/bin/bash

# Author: Authmane Terki (authmane512)
# E-mail: authmane512 (at) protonmail.ch
# Blog: https://medium.com/@authmane512
# Source: https://github.com/authmane512/android-project-template
# Tutorial: https://medium.com/@authmane512/how-to-do-android-development-faster-without-gradle-9046b8c1cf68
# This project is on public domain
#
# Hello! I've made this little script that allow you to init, compile and run an Android Project.
# I tried to make it as simple as possible to allow you to understand and modify it easily.
# If you think there is a very important missing feature, don't hesitate to do a pull request on Github and I will answer quickly.
# Thanks! 

set -e

APP_NAME="زومیلا (آژانس املاک)"
PACKAGE_NAME="com.gordarg.app"

AAPT="/home/tayyebi/AndroidSDK/build-tools/28.0.3/aapt"
DX="/home/tayyebi/AndroidSDK/build-tools/28.0.3/dx"
#DX="/home/tayyebi/AndroidSDK/build-tools/28.0.3/d8"
ZIPALIGN="/home/tayyebi/AndroidSDK/build-tools/28.0.3/zipalign"
APKSIGNER="/home/tayyebi/AndroidSDK/build-tools/28.0.3/apksigner"
PLATFORM="/home/tayyebi/AndroidSDK/platforms/android-14/android.jar"

build() {
	echo "Cleaning..."
	rm -rf obj/*
	rm -rf "$PACKAGE_DIR/R.java"

	echo "Generating R.java file..."
	$AAPT package -f -m -J src -M AndroidManifest.xml -S res -I $PLATFORM

	echo "Compiling..."
	ant compile -Dplatform=$PLATFORM

	echo "Making APK..."
	$AAPT package -f -m -F bin/app.unaligned.apk -M AndroidManifest.xml -S res -I $PLATFORM


	echo "Translating in Dalvik bytecode..."
	$DX --dex --output=classes.dex obj
	$AAPT add bin/app.unaligned.apk classes.dex

	echo "Signing APK"

# V1 Signing
#  jarsigner -verbose -sigalg SHA1withRSA -digestalg SHA1 -keystore release.keystore -storepass "123456" -keypass "123456" bin/app.unaligned.apk alias_name

# V2 Signing
  $APKSIGNER sign --ks release.keystore --v1-signing-enabled true --v2-signing-enabled false --ks-pass "pass:123456" bin/app.unaligned.apk


  echo "Aligning APK"
	$ZIPALIGN -f 4 bin/app.unaligned.apk bin/app.apk

	echo "Verifying"
	$APKSIGNER version
	$APKSIGNER verify --min-sdk-version 28 --print-certs -v bin/app.apk
}

run() {
	echo "Launching..."
	adb install -r bin/app.apk
	adb shell am start -n "${PACKAGE_NAME}/.MainActivity"
}

PACKAGE_DIR="src/$(echo ${PACKAGE_NAME} | sed 's/\./\//g')"

case $1 in
	init)
		init
		;;
	build)
		build
		;;
	run)
		run
		;;
	build-run)
		build
		run
		;;
	*)
		echo "error: unknown argument"
		;;
esac
