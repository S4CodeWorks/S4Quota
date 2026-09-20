# S4Quota — Codex App Server provider spike

Harness isolado Node.js para validar o Codex App Server no Windows. Não é a
fundação do aplicativo S4Quota e não contém Tauri, React, UI ou Design System.

Usa apenas APIs nativas do Node e nunca lê arquivos de autenticação do Codex nem
chama endpoints privados. A saída ao vivo é resumida: não persiste e-mail, IDs
de conta, percentuais exatos, timestamps de reset, tokens ou payloads brutos.

```powershell
npm test
npm run probe
npm run observe -- 20
npm run cross-process
```

`probe` valida discovery, handshake, conta e formato de quota. `observe` aguarda
notificações sem criar uso. `cross-process` executa um turno mínimo read-only e
verifica se uma conexão independente recebe atualização de rate limit.
