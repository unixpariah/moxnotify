#!/home/unixpariah/.nix-profile/bin/nu

#for x in 0..10 {
#     notify-send "Title" $"Notification ($x)" -h $"int:value:($x * 10)" --icon=zen-beta -h string:image-path:kitty
#}
notify-send "Title" "Notification" -h "int:value:10" --icon=zen-beta -h string:image-path:kitty -A default,Open -A cancel,cancel
