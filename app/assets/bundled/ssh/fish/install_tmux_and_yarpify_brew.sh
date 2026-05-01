brew install tmux

if test $status -eq 0
    tmux -Lyarp -CC
    exit
end
