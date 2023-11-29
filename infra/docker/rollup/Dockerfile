ARG OCTEZ_TAG
FROM tezos/tezos:${OCTEZ_TAG} AS octez

FROM alpine:3.15 AS rollup
RUN apk --no-cache add binutils bash gcc gmp libgmpxx hidapi libc-dev libev libffi sudo
ARG OCTEZ_PROTO
COPY --from=octez /usr/local/bin/octez-smart-rollup-node-${OCTEZ_PROTO} /usr/bin/octez-smart-rollup-node
COPY --from=octez /usr/local/bin/octez-client /usr/bin/octez-client
COPY ./build/jstz_kernel_installer.hex /root/kernel.hex
COPY ./build/preimages/ /root/preimages/
COPY ./infra/docker/rollup/entrypoint.sh .
RUN chmod +x entrypoint.sh
ENTRYPOINT [ "./entrypoint.sh" ]