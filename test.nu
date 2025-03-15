#!/home/unixpariah/.nix-profile/bin/nu

for x in 0..10 {
     notify-send "Title" $"Notification ($x)" -h $"int:value:($x * 10)" --icon=zen-beta -h string:image-path:kitty
}
