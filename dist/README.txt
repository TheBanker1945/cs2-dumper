UNDERBOSS - overlay for Counter-Strike 2
========================================

QUICK START
-----------
1. Unzip this folder anywhere (Desktop is fine). Keep UnderBoss.exe in it.
2. Double-click UnderBoss.exe. You can do this before or after starting CS2:
   if CS2 isn't running yet, UnderBoss waits for it and attaches by itself.
3. Switch to CS2. The ESP shows as soon as you're in a match.
4. Press INSERT in game to open the menu.

Nothing to install, and no other files are needed. UnderBoss reads fresh
offsets from the game every time it starts.


FIRST LAUNCH: "WINDOWS PROTECTED YOUR PC"
-----------------------------------------
UnderBoss isn't signed, so Windows SmartScreen warns about it the first time.
Click "More info", then "Run anyway". Some antivirus programs also flag tools
that read another program's memory; you may have to allow it there too.


CONTROLS (only while CS2 is the active window)
----------------------------------------------
  INSERT            Open / close the menu
  UP / DOWN         Select a row in the menu
  ENTER, LEFT/RIGHT Switch the selected row on or off
  END               Quit UnderBoss
  Mouse 4 (hold)    Trigger Bot, once it's switched on in the menu

The black console window shows UnderBoss's status. Leave it open (minimizing
it is fine); closing it closes UnderBoss. UnderBoss also quits by itself when
CS2 closes.


MENU
----
  Skeleton      Draws each player's skeleton, in team colours.
  Health Bar    Health bar next to each player.
  Weapon        Held weapon and ammo under each player.
  Name          Player name above each head ("BOT" in front of bots).
  Trigger Bot   Off by default. While you hold Mouse 4, pulls your crosshair
                onto the head of an enemy in clear view and fires. While it's
                on, shooting by hand also pulls onto the head.

Your menu choices are saved and come back next time you start UnderBoss
(they're kept in %APPDATA%\UnderBoss, so a new version keeps them too).


TROUBLESHOOTING
---------------
Nothing shows on screen
  - Click into CS2. UnderBoss hides whenever CS2 isn't the active window.
  - The ESP only draws players while you're in a match, not in the main menu.

"Couldn't read offsets from CS2"
  - CS2 probably updated and changed its internals. Ask for a newer UnderBoss.
  - If CS2 was still starting up, just start UnderBoss again.

"Access is denied" or "Run UnderBoss as Administrator"
  - Right-click UnderBoss.exe and choose "Run as administrator".

The ESP is offset from the players
  - Make sure CS2's resolution matches the monitor in Fullscreen mode, or use
    "Fullscreen Windowed". Restart UnderBoss after changing it.


WARNING
-------
Valve's anti-cheat (VAC) can detect tools like this. Using it on official
matchmaking or other VAC-secured servers can get your Steam account
permanently banned. Use it at your own risk.
