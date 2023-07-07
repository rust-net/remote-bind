# OnePort

Reuse ports based on probe request data

通过探测请求数据实现端口复用


# run
```
$ oneport
$ # or
$ oneport -c /path/to/config.yml
```


# config.yml
该文件配置如何反向代理 Socket 请求：
```
config:
  # 监听地址
  listen: 0.0.0.0:1111

rules:
    # 字符串匹配
  - rule: GET
    address: 127.0.0.1:80
    # 首字节匹配
  - rule: 3
    address: 127.0.0.1:3389
    # 多字节匹配
  - rule: [0x53, 0x53]
    address: 127.0.0.1:22
    # 内置规则
  - rule: $SSH
    address: 127.0.0.1:22
    # 内置规则可能是多个规则的集合，比如 $HTTP 匹配 GET、POST 等多个字符串
  - rule: $HTTP
    address: 127.0.0.1:80
    # 通配规则
  - rule: []
    address: 127.0.0.1:4444
```
目前的内置规则有：`$SSH`, `$RDP`, `$HTTP`

规则按顺序匹配，越靠前优先级越高


# hot-reload
配置修改后支持热重载：
```
$ oneport -r
$ # or
$ oneport --reload
```


# usage
![image](https://github.com/develon2015/remote-bind/assets/27133157/b6326901-510d-41ac-80ae-46983c91a8b9)
![image](https://github.com/develon2015/remote-bind/assets/27133157/366e0194-9cb9-47b1-ab70-ab9a9d13af8d)
