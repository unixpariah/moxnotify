{ lib, config, ... }:
with config.lib.stylix.colors.withHashtag;
with config.stylix.fonts;
let
  moxnotifyOpacity = lib.toHexString (
    ((builtins.floor (config.stylix.opacity.popups * 100 + 0.5)) * 255) / 100
  );
in
{
  options.stylix.targets.moxnotify.enable = config.lib.stylix.mkEnableTarget "moxnotify" true;

  config = lib.mkIf (config.stylix.enable && config.stylix.targets.moxnotify.enable) {
    services.moxnotify.settings = {
      #prev = {
      #background = base01 + moxnotifyOpacity;
      #border.color = base0E;
      #};
      #next = {
      #background = base01 + moxnotifyOpacity;
      #border.color = base0E;
      #};

      styles = [
        {
          selector = "notification";
          style = {
            background = {
              urgency_low = base00 + moxnotifyOpacity;
              urgency_normal = base01 + moxnotifyOpacity;
              urgency_critical = base01 + moxnotifyOpacity;
            };

            font = {
              family = sansSerif.name;
              size = sizes.popups;
              color = base05;
            };

            border.color = {
              urgency_low = base0B;
              urgency_normal = base0E;
              urgency_critical = base08;
            };
          };
        }
        {
          selector = "notification";
          state = "hover";
          style.background = {
            urgency_low = base02 + moxnotifyOpacity;
            urgency_normal = base02 + moxnotifyOpacity;
            urgency_critical = base02 + moxnotifyOpacity;
          };
        }
      ];

      #styles = {
      #default = {
      #progress = {
      #complete_color = {
      #urgency_low = base0F;
      #urgency_normal = base0F;
      #urgency_critical = base08;
      #};
      #border.color = {
      #urgency_low = base0B;
      #urgency_normal = base0E;
      #urgency_critical = base08;
      #};
      #};

      #buttons = {
      #action = {
      #default = {
      #background = base01;
      #font.color = base05;
      #border.color = {
      #urgency_low = base0B;
      #urgency_normal = base0E;
      #urgency_critical = base08;
      #};
      #};
      #hover = {
      #background = base0F;
      #font.color = base05;
      #};
      #};
      #};

      #background = {
      #urgency_low = base00 + moxnotifyOpacity;
      #urgency_normal = base01 + moxnotifyOpacity;
      #urgency_critical = base01 + moxnotifyOpacity;
      #};

      #icon.border.color = {
      #urgency_low = base0B;
      #urgency_normal = base0E;
      #urgency_critical = base08;
      #};

      #font = {
      #family = sansSerif.name;
      #size = sizes.popups;
      #color = base05;
      #};

      #border.color = {
      #urgency_low = base0B;
      #urgency_normal = base0E;
      #urgency_critical = base08;
      #};
      #};
      #hover = {
      #background = {
      #urgency_low = base02 + moxnotifyOpacity;
      #urgency_normal = base02 + moxnotifyOpacity;
      #urgency_critical = base02 + moxnotifyOpacity;
      #};

      #buttons.dismiss = {
      #default = {
      #background = base08 + "aa";
      #border.color = base08 + "aa";
      #};
      #hover = {
      #background = base08;
      #border.color = base08;
      #};
      #};
      #};
      #};
    };
  };
}
