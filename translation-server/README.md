# DFI18N 腾讯云翻译服务

部署在腾讯云轻量应用服务器上的《矮人要塞》翻译后端。客户端只提交英文文本；Prompt、DeepSeek 密钥和翻译缓存都保留在服务器端。

## 腾讯云部署

推荐使用 Ubuntu 22.04/24.04 的 Docker 应用镜像，并准备一个解析到服务器公网 IP 的域名。

```bash
cp .env.example .env
nano .env
docker compose up -d --build
```

必须在 `.env` 中填写：

- `SITE_ADDRESS`：有域名时填写 `https://translate.example.com`；临时使用公网 IP 时填写 `http://公网IP`。
- `UPSTREAM_API_KEY`：DeepSeek API Key。

在腾讯云防火墙中放行 TCP 80 和 443。Caddy 会自动申请和续期 HTTPS 证书。

验证：

```bash
curl https://translate.example.com/health
curl -X POST https://translate.example.com/v1/translate \
  -H 'Content-Type: application/json' \
  -H 'X-DFI18N-Client: dfi18n-runtime-v1' \
  -d '{"text":"He is quick to anger."}'
```

然后把客户端 `cloud-translation.toml` 的 `endpoint` 改为：

```toml
endpoint = "https://translate.example.com/v1/translate"
```

## 数据与维护

- SQLite 缓存保存在 Docker 卷 `translation-data` 中。
- 相同模型、Prompt 版本和原文使用同一个缓存键。
- 相同句子的并发请求会合并成一次上游调用。
- 修改 Prompt 后增加 `PROMPT_VERSION`，旧缓存会自然失效但不会删除。

查看日志和更新服务：

```bash
docker compose logs -f api
docker compose up -d --build
```

接口保持兼容原 Cloudflare Worker：

```http
POST /v1/translate
X-DFI18N-Client: dfi18n-runtime-v1
Content-Type: application/json

{"text":"He is quick to anger."}
```
