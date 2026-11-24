# Angel One · app de bordo

O convés é a entrada dos passageiros. Quatro toques rápidos nas asas do logo abrem a escolha de tripulante. O modo fica salvo no aparelho; para sair, abra a engrenagem no convés.

## Prévia nesta estação

Na pasta `app`, use `bash preview.sh build`, depois `bash preview.sh start`. `bash preview.sh status` mostra o endereço HTTPS temporário. `bash preview.sh stop` encerra o túnel e o servidor. O endereço público pode mudar se o túnel for reiniciado; a base da prévia permanece em `~/.local/share/angel-one/preview`. A publicação serve `web/dist`, não a pasta do projeto.

## Desenvolvimento

- `web`: Svelte + TypeScript. `npm ci`, `npm run check`, `npm run build`. Para edição com recarga, `npm run dev` usa o proxy configurado em `vite.config.ts`.
- Imagens: os uploads abrem uma tela de recorte antes do envio; escolher **Imagem inteira** pula o recorte. O original é salvo, e a IA converte em ilustração 2D. A versão discreta aparece no rodapé da tela inicial (fonte: `web/package.json`).
- `server`: Rust/Axum, SQLite, mídia local, API e notificações. A partir de `app`: `DATA_DIR=./data WEB_DIR=./web/dist PORT=8080 cargo run --release --manifest-path server/Cargo.toml`. A IA usa `OPENROUTER_API_KEY` **no ambiente do servidor**. O cadastro manual funciona sem ela.
- `routing`: malha de água de Jurerê e arredores. As distâncias consideram a costa do OpenStreetMap; não consideram calado, pedras ou profundidade. Fora da malha aparece a distância em linha reta.
- `server/smoke.sh`: verifica os fluxos da API numa base descartável, sem fazer geração paga. Os testes de clima e catálogo de modelos precisam de internet.
- Chave para a prévia local: `~/.config/angel-one/preview.env`, permissões `0600`, uma linha `OPENROUTER_API_KEY=...`. Não coloque a chave no projeto nem no navegador. As capas 2D são geradas uma única vez com `python3 -I tools/generate_covers.py <faixa>`.
- `API.md`: contrato entre app e servidor. `../backlog.js`: decisões e acompanhamento.

## Publicar no Unraid

O workflow `.github/workflows/docker.yml` testa e publica `ghcr.io/luizof/angel-one:latest` a cada push em `main` e uma tag imutável `sha-...`. A imagem é apenas `linux/amd64` (Unraid x86_64), com Rust musl, frontend pronto e base Alpine pequena (inclui libssl para Web Push).

No formulário **Add Container** do Unraid:

1. **Repository:** `ghcr.io/luizof/angel-one:latest`. Se o pacote aparecer como privado na primeira publicação, altere a visibilidade para **Public** no GitHub antes de puxá-lo sem login.
2. **Network port:** host `8080` → container `8080` (TCP). Use este endereço como serviço de origem no túnel cloudflared já configurado no Unraid.
3. **Path (persistência):** host `/mnt/user/appdata/angel-one` → container `/data` (leitura e escrita). O servidor roda como UID/GID **10001**. Crie o diretório e conceda permissão de escrita a 10001 antes de iniciar; não use `/data` dentro da imagem para dados permanentes.
4. **Environment variable (secreta):** `OPENROUTER_API_KEY`. Cole o valor **só** no campo de variável de ambiente do Unraid, nunca no GitHub, na imagem ou no frontend. Sem ela, o cadastro manual funciona, mas as funções de IA ficam desligadas.
5. Opcionalmente defina `PORT=8080` (padrão), `TZ_NAME=America/Sao_Paulo`. A previsão **não depende** do GPS do visitante: usa sempre o ponto fixo ao norte de Jurerê Internacional.

O arquivo `compose.unraid.yml` contém a mesma configuração. Antes de usá-lo, defina `OPENROUTER_API_KEY` no ambiente do Compose **no Unraid** (ou arquivo `.env` privado nesse servidor). O volume guarda o SQLite, as fotos e o par VAPID das notificações; faça backup de `/mnt/user/appdata/angel-one`.

## Teste no iPhone

1. No Safari, toque quatro vezes no logo e cadastre seu nome. Em **Despensa**, crie um grupo em litros e um item com conteúdo de 750 ml e quantidade fracionária. Confira o total do grupo, registre uma compra e faça a conferência.
2. Em **Destinos**, marque um ponto no mar, selecione o abrigo do vento e salve. Em outro aparelho, vote e confira a contagem no primeiro. Permita localização para ver a distância.
3. No **Bar**, cadastre um item e faça um pedido como passageiro indicando quem vai receber. Em **Comandas**, passe de recebido para preparando e pronto. O pedido não altera o estoque.
4. Para testar push, instale pelo menu **Compartilhar → Adicionar à Tela de Início**. Abra pelo ícone e habilite os avisos. Use o mesmo link nos dois aparelhos; permissões e inscrições pertencem àquele endereço.
