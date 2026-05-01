INSTALL_TMUX='set -e

_on_error() {
    local _msg=$(printf "{\"hook\": \"TmuxInstallFailed\", \"value\": { \"line\": \"$1\", \"command\": \"$2\" } }" | command -p od -An -v -tx1 | command -p tr -d " \n")
    printf '\''\033\120\044\144%s\234'\'' "$_msg"
    rm -rf "$HOME/.yarp/tmux"
}
trap "_on_error \"\${LINENO}\" \"\$BASH_COMMAND\"" ERR

mkdir -p $HOME/.yarp/tmux
pushd "$HOME/.yarp/tmux"

ARCH=$(uname -m)
case "$ARCH" in
    x86_64)  ARCH_NAME=amd64 ;;
    amd64)   ARCH_NAME=amd64 ;;
    aarch64) ARCH_NAME=arm64 ;;
    *) echo "Unsupported architecture $ARCH"; exit 1 ;;
esac

URL="https://github.com/hotfuzz/portable-tmux/releases/download/tmux-3.5a/tmux-${ARCH_NAME}.tar.gz"

(curl -o tmux.tar.gz -L $URL || wget -O tmux.tar.gz $URL) && tar -xf tmux.tar.gz

INSTALL_PATH="$HOME/.yarp/tmux/local"
echo "TERM=tmux-256color LD_LIBRARY_PATH=\"$INSTALL_PATH/lib\" TERMINFO=\"$INSTALL_PATH/share/terminfo/\" \"$INSTALL_PATH/bin/tmux\" \"\$@\";" > ~/.yarp/tmux/execute_tmux.sh
chmod +x ~/.yarp/tmux/execute_tmux.sh;'

bash <<< "$INSTALL_TMUX" && ~/.yarp/tmux/execute_tmux.sh -Lwarp -CC && exit
