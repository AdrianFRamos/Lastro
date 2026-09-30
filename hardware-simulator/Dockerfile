FROM python:3.13.15-slim-bookworm@sha256:ed86c82274b3c69b52fb5820f358f0bd7df0b603332063cb5c6e32bd220c3e6e

ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    PYTHONPATH=/app/src

WORKDIR /app
COPY hardware-simulator/requirements.txt ./requirements.txt
RUN python -m pip install --no-cache-dir -r requirements.txt

COPY hardware-simulator/src ./src

RUN useradd --create-home --uid 10001 --shell /usr/sbin/nologin lastro-sim \
    && chown -R lastro-sim:lastro-sim /app
USER lastro-sim

EXPOSE 8090 9100
HEALTHCHECK --interval=5s --timeout=2s --start-period=5s --retries=12 \
  CMD ["python", "-c", "import urllib.request; urllib.request.urlopen('http://127.0.0.1:8090/healthz', timeout=1).read()"]

ENTRYPOINT ["python", "-m", "lastro_hardware_simulator.server"]
