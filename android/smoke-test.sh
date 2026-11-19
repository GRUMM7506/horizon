#!/usr/bin/env bash
# A smoke test of the Android port on an emulator (docs/android-port.md): the library loads, the
# game's thread has its stack, a window surface opens, and the game survives its activity stopping
# and coming back. No file of Minecraft's is on the emulator, so the game stops at its
# missing-resources screen: that is the engine's own window, renderer and event loop, without a
# world. A real GPU and the game itself are for a phone to test.
#
#   android/smoke-test.sh <apk> [output folder]
#
# Needs ANDROID_HOME with the emulator and the system image named below, and KVM.
set -uo pipefail

apk=${1:?the APK to test}
out=${2:-smoke-out}
image=${SMOKE_IMAGE:-system-images;android-34;default;x86_64}
package=dev.horizonport.android
folder=/storage/emulated/0/Android/data/$package/files/horizon
sdk=${ANDROID_HOME:?ANDROID_HOME is not set}
adb_tool=$sdk/platform-tools/adb
failed=0

# No command to the emulator may hold the job: each one has a minute, the install five.
adb() {
    timeout "${ADB_TIMEOUT:-60}" "$adb_tool" "$@"
}

mkdir -p "$out"
fail() {
    echo "FAILED: $*"
    failed=1
}
alive() {
    # The game's own process, beside the launcher's.
    adb shell pidof "$package:game" > /dev/null 2>&1
}
shot() {
    adb exec-out screencap -p > "$out/$1.png" 2> /dev/null || true
    echo "screenshot $1: $(wc -c < "$out/$1.png") bytes"
}
game_log() {
    adb shell "cat $folder/logs/$1" 2> /dev/null
}

echo "== Starting the emulator"
# One folder for both tools: avdmanager would follow XDG_CONFIG_HOME where it is set, and the
# emulator would not find the device there.
export ANDROID_AVD_HOME=${ANDROID_AVD_HOME:-$HOME/.android/avd}
mkdir -p "$ANDROID_AVD_HOME"
echo no | "$sdk/cmdline-tools/latest/bin/avdmanager" create avd --force --name horizon-smoke \
    --package "$image" --device pixel_5 || { echo "FAILED: the emulator's device was not made"; exit 1; }
"$sdk/emulator/emulator" -list-avds
"$sdk/emulator/emulator" -avd horizon-smoke -no-window -no-audio -no-boot-anim -no-snapshot \
    -gpu swiftshader_indirect -memory 4096 -cores 2 > "$out/emulator.log" 2>&1 &
emulator=$!
trap 'kill $emulator 2> /dev/null' EXIT
# An emulator that never came up must not hold the job until it times out.
if ! ADB_TIMEOUT=300 adb wait-for-device; then
    echo "FAILED: the emulator did not start"
    tail -n 80 "$out/emulator.log"
    exit 1
fi
booted=0
for _ in $(seq 120); do
    if [ "$(adb shell getprop sys.boot_completed 2> /dev/null | tr -d '\r')" = 1 ]; then
        booted=1
        break
    fi
    sleep 5
done
if [ $booted != 1 ]; then
    echo "FAILED: the emulator did not boot"
    tail -n 50 "$out/emulator.log"
    exit 1
fi
# The game's activity is not exported: only root may start it directly.
adb root > /dev/null
sleep 3
ADB_TIMEOUT=120 adb wait-for-device || { echo "FAILED: the emulator is gone after adb root"; exit 1; }
adb shell input keyevent KEYCODE_WAKEUP
adb shell wm dismiss-keyguard

echo "== Installing"
ADB_TIMEOUT=300 adb install -r "$apk" || { echo "FAILED: the APK did not install"; exit 1; }
# The launcher makes the game's folder, as on a phone.
adb shell am start -n "$package/.LauncherActivity" > /dev/null
sleep 8
shot 1-launcher
adb logcat -c

echo "== Starting the game (no resources: its missing-resources screen)"
adb shell am start -n "$package/.HorizonActivity" \
    --esa args "--game-dir,$folder,--home,$folder,--resources,$folder/none,--name,Smoke" > /dev/null
sleep 60
shot 2-game
alive || fail "the game's process is gone after starting"

echo "== Touching the screen"
adb shell input tap 300 300
adb shell input swipe 400 400 800 500 300
sleep 3
alive || fail "the game's process is gone after touches"

echo "== Leaving the game (Home) and coming back"
adb shell input keyevent KEYCODE_HOME
sleep 10
alive || fail "the game's process is gone after Home"
adb shell monkey -p "$package" -c android.intent.category.LAUNCHER 1 > /dev/null 2>&1
sleep 15
shot 3-back
alive || fail "the game's process is gone after coming back"

echo "== Once more"
adb shell input keyevent KEYCODE_HOME
sleep 6
adb shell monkey -p "$package" -c android.intent.category.LAUNCHER 1 > /dev/null 2>&1
sleep 10
shot 4-back-again
alive || fail "the game's process is gone after the second return"

echo "== Logs"
adb logcat -d > "$out/logcat.txt" 2>&1
game_log android-output.log > "$out/android-output.log"
game_log latest.log > "$out/latest.log"
adb shell "cat $folder/last-exit.txt" > "$out/last-exit.txt" 2> /dev/null
echo "--- android-output.log"
cat "$out/android-output.log"
echo "--- latest.log (the end)"
tail -n 80 "$out/latest.log"
echo "--- logcat: the game's tag, crashes"
grep -E " Horizon *:|FATAL EXCEPTION|Fatal signal|AndroidRuntime: |libhorizon_android|GameActivity" "$out/logcat.txt" | tail -n 120
echo "--- last-exit.txt: $(cat "$out/last-exit.txt")"

# The game's log is a file, or its standard error when the file could not be had.
cat "$out/latest.log" "$out/android-output.log" > "$out/all.log"
grep -q "thread stack 136 MiB" "$out/android-output.log" ||
    fail "the game's thread did not get its stack (android-output.log)"
grep -q "the activity stopped" "$out/all.log" ||
    fail "the game never heard its activity stop"
grep -q "the activity is back" "$out/all.log" ||
    fail "the game never got its window back"
grep -qE "Fatal signal|FATAL EXCEPTION" "$out/logcat.txt" && grep -E "Fatal signal|FATAL EXCEPTION" -A 30 "$out/logcat.txt" | grep -q "$package" &&
    fail "a crash of the app is in logcat"

if [ $failed = 0 ]; then
    echo "== The smoke test passed"
fi
exit $failed
