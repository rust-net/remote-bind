# remote-bind
将本地服务映射到公网

## Install
Windows / Linux: [releases](https://github.com/rust-net/remote-bind/releases)

Android: [remote-bind-apk](https://github.com/rust-net/remote-bind-apk)

## 服务器中继
在服务器 `x.x.x.x` 的 `1234` 端口上运行server，并设置密码为 `passwd` ：
```
$ ./server 1234 passwd
2023-09-04 00:00:00 - [I] - server/src/main.rs:76 -> Server started on x.x.x.x:1234
2023-09-04 00:00:00 - [I] - core/src/server.rs:71 -> STUN started on 0.0.0.0:1234
2023-09-04 00:00:00 - [I] - core/src/server.rs:71 -> STUN started on 0.0.0.0:1235
```

在局域网主机上运行客户端，将 `127.0.0.1:3389` 服务映射到 `x.x.x.x:13389` 上：
```
$ ./client x.x.x.x:1234 13389 passwd 127.0.0.1:3389
2023-09-04 00:00:00 - [I] - client\src\main.rs:97 -> 正在连接服务器：x.x.x.x:1234
2023-09-04 00:00:00 - [I] - core\src\client.rs:18 -> 正在连接
2023-09-04 00:00:00 - [I] - core\src\client.rs:20 -> 连接完成
2023-09-04 00:00:00 - [I] - client\src\main.rs:104 -> 正在绑定端口：13389
2023-09-04 00:00:00 - [I] - client\src\main.rs:108 -> 服务已绑定: 127.0.0.1:3389 -> x.x.x.x:13389
```

## P2P直连
在要进行P2P访问的主机上运行客户端，监听 `127.0.0.1:9833` 地址，映射到 `x.x.x.x:13389` 上绑定的服务：
```
$ ./client p2p x.x.x.x:1234 13389 127.0.0.1:9833
2023-09-04 00:00:00 - [I] - core\src\client_p2p.rs:27 -> 正在测试
2023-09-04 00:00:00 - [I] - core\src\client_p2p.rs:29 -> 测试成功
2023-09-04 00:00:00 - [I] - core\src\client_p2p.rs:31 -> 服务已启动: 127.0.0.1:9833
```
> 注意：P2P无法保证100%的成功率