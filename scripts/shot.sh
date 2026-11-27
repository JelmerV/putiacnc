#!/bin/sh
# Take a screenshot of the window in a given state, on a virtual screen (Linux; needs xvfb and imagemagick).
#
#   scripts/shot.sh [options] [project.json]
#
#   -o FILE     where to write the picture (default shot.png)
#   -w WxH      window size (default 1920x985; the smallest the app allows is about 1354x640)
#   -t THEME    light or dark (default light)
#   -c X,Y,W,H  keep only this part of the screen (default: the whole window)
#   -s SECONDS  how long to wait before the picture (default 10; toolpath checks need up to 60)
#   -e VAR=VAL  an extra PUTIACNC_* setting, e.g. -e PUTIACNC_VIEW=3d (can be repeated; see docs/testing.md)
#   -b BINARY   the program (default $CARGO_TARGET_DIR/debug/putiacnc, else target/debug/putiacnc)
#
# Exits 1 and writes no picture if the program dies before the picture is taken, or if the whole screen is one colour
# (the window never drew); the program's last output is printed.
# It always uses its own virtual X11 screen (:99, or $SHOT_DISPLAY), so it works the same under Wayland.
#
# Examples:
#   scripts/shot.sh -e PUTIACNC_VIEW=3d -e PUTIACNC_PLAY=0.6 -o 3d.png tests/data/outside_holes.project.json
#   scripts/shot.sh -t dark -w 1354x800 -c 396,600,534,200 -o bars.png my.project.json
set -e
out=shot.png; win=1920x985; theme=light; crop=""; wait=10; bin=""; envs=""
while getopts "o:w:t:c:s:e:b:h" o; do
  case $o in
    o) out=$OPTARG ;; w) win=$OPTARG ;; t) theme=$OPTARG ;; c) crop=$OPTARG ;; s) wait=$OPTARG ;;
    e) envs="$envs $OPTARG" ;; b) bin=$OPTARG ;;
    *) sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
  esac
done
shift $((OPTIND - 1))
proj=${1:-}
[ -n "$bin" ] || bin="${CARGO_TARGET_DIR:-target}/debug/putiacnc"
[ -x "$bin" ] || { echo "No program at $bin. Build it first: cargo build -p putiacnc-gui" >&2; exit 1; }
command -v import >/dev/null || { echo "ImageMagick's 'import' is missing." >&2; exit 1; }

# its own virtual screen (X11), started once and left running. The desktop's DISPLAY and WAYLAND_DISPLAY are ignored on
# purpose: on Wayland the app would otherwise open on your real desktop, and a screenshot of an X11 root shows nothing.
vd=${SHOT_DISPLAY:-:99}
if [ ! -S "/tmp/.X11-unix/X${vd#:}" ] || ! DISPLAY=$vd xdpyinfo >/dev/null 2>&1; then
  command -v Xvfb >/dev/null || { echo "Xvfb is missing (on Arch: sudo pacman -S xorg-server-xvfb)." >&2; exit 1; }
  rm -f "/tmp/.X${vd#:}-lock" "/tmp/.X11-unix/X${vd#:}"
  nohup Xvfb "$vd" -screen 0 1920x1080x24 >/dev/null 2>&1 &
  sleep 3
fi

# a throwaway config folder, so saved preferences never change the picture
cfg=${PUTIACNC_CONFIG_DIR:-$(mktemp -d)}
tmpcfg=""; [ -n "$PUTIACNC_CONFIG_DIR" ] || tmpcfg=$cfg
# the app's output goes to a log that is shown when something goes wrong
log=$(mktemp)
trap 'rm -rf "$log" $tmpcfg' EXIT
fail() { echo "error: $1. No picture written. The program's last output:" >&2; tail -n 15 "$log" >&2; rm -f "$out"; exit 1; }

# never leave an old picture behind that could pass for this run's
rm -f "$out"
# shellcheck disable=SC2086
env -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE DISPLAY="$vd" WINIT_UNIX_BACKEND=x11 PUTIACNC_CONFIG_DIR="$cfg" PUTIACNC_SKIP_SETUP=${PUTIACNC_SKIP_SETUP-1} PUTIACNC_THEME="$theme" PUTIACNC_WIN="$win" $envs "$bin" $proj >"$log" 2>&1 &
pid=$!
sleep "$wait"
# a program that crashed at start (missing library, panic) is an error, not a black picture
if ! kill -0 $pid 2>/dev/null; then
  code=0; wait $pid || code=$?
  fail "$bin exited (code $code) before the picture was taken"
fi
if [ -n "$crop" ]; then
  IFS=, read -r x y w h <<EOF
$crop
EOF
  DISPLAY=$vd import -window root -crop "${w}x${h}+${x}+${y}" +repage "$out"
else
  DISPLAY=$vd import -window root "$out"
fi
kill $pid 2>/dev/null || true
wait $pid 2>/dev/null || true
# a whole screen of one colour means the window never drew (a crop of plain background may be one colour, so not checked)
if [ -z "$crop" ] && [ "$(convert "$out" -format %k info:)" -le 1 ]; then
  fail "the picture is a single colour: the window never drew"
fi
echo "$out"
