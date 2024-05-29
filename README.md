![logo](assets/superman.png)

:rotating_light: Driver have been blocked in latest Windows 10/11!

# superman

Kill The Protected Process

> This tool is limited to security research and teaching, and the user bears all legal and related responsibilities caused by the use of this tool! The author does not assume any legal and related responsibilities!

## usage

```shell
Options:
  -p, --pid <PID>    Pid to kill
  -r                 Recursive kill process
  -t, --time <TIME>  Kill interval time (milliseconds) [default: 500]
  -h, --help         Print help
  -V, --version      Print version
```

Kill Windows Defender (MsMpEng.exe)

```shell
superman.exe -p <PID> -r
```

![demo](assets/demo.gif)
