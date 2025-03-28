# today-google-calendar-viewer

# リモート先で使っているスクリプト

```kill-gnome-terminal.sh
#!/bin/bash

# gnome-terminalプロセスのPIDを取得し、最初の1つだけkillする
ps -ef | grep gnome-terminal | grep -v grep | awk '{print $2}' | head -n 1 | xargs kill -9
```

```rotate-display.sh
wlr-randr --output HDMI-A-1 --transform 90
```

```start-calendar.sh
#!/bin/bash

gnome-terminal --full-screen -- bash -c "~/today-google-calendar; bash"
```

```restart-calendar.sh
#!/bin/bash

# gnome-terminalプロセスのPIDを取得し、最初の1つだけkillする
ps -ef | grep gnome-terminal | grep -v grep | awk '{print $2}' | head -n 1 | xargs kill -9
gnome-terminal --zoom=1.7 --full-screen -- bash -c "~/today-google-calendar; bash"
```