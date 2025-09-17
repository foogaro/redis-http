# Redis HTTP Module Docker Image
# Based on Ubuntu 24.04 with Redis 8.2.1 and the Redis HTTP module

FROM ubuntu:24.04

# Set environment variables
ENV DEBIAN_FRONTEND=noninteractive
ENV REDIS_VERSION=8.2.1
ENV REDIS_USER=redis
ENV REDIS_GROUP=redis
ENV REDIS_HOME=/var/lib/redis
ENV REDIS_LOG_DIR=/var/log/redis
ENV REDIS_CONF_DIR=/etc/redis

# Install system dependencies
RUN apt-get update && apt-get install -y \
    wget \
    curl \
    build-essential \
    pkg-config \
    libssl-dev \
    tcl \
    && rm -rf /var/lib/apt/lists/*

# Create redis user and directories
RUN groupadd -r ${REDIS_GROUP} && \
    useradd -r -g ${REDIS_GROUP} -d ${REDIS_HOME} -s /sbin/nologin ${REDIS_USER} && \
    mkdir -p ${REDIS_HOME} ${REDIS_LOG_DIR} ${REDIS_CONF_DIR} && \
    chown -R ${REDIS_USER}:${REDIS_GROUP} ${REDIS_HOME} ${REDIS_LOG_DIR} ${REDIS_CONF_DIR}

# Download and install Redis 8.2.1
RUN cd /tmp && \
    wget http://download.redis.io/releases/redis-${REDIS_VERSION}.tar.gz && \
    tar xzf redis-${REDIS_VERSION}.tar.gz && \
    cd redis-${REDIS_VERSION} && \
    make && \
    make install && \
    cd / && \
    rm -rf /tmp/redis-${REDIS_VERSION}*

# Create Redis configuration directory for modules
RUN mkdir -p /usr/lib/redis/modules && \
    chown ${REDIS_USER}:${REDIS_GROUP} /usr/lib/redis/modules

# Download the Redis HTTP module from GitHub releases
# Note: Replace 'your-repo' with your actual GitHub repository
ARG GITHUB_REPO=your-repo/redis-http
ARG RELEASE_TAG=latest
RUN cd /usr/lib/redis/modules && \
    wget -O libredis_http.so "https://github.com/${GITHUB_REPO}/releases/${RELEASE_TAG}/download/libredis_http.so" && \
    chown ${REDIS_USER}:${REDIS_GROUP} libredis_http.so && \
    chmod 755 libredis_http.so

# Create Redis configuration file
RUN cat > ${REDIS_CONF_DIR}/redis.conf << 'EOF'
# Redis 8.2.1 Configuration for Redis HTTP Module

# Network
bind 0.0.0.0
port 6379
protected-mode no

# General
daemonize no
supervised no
pidfile /var/run/redis_6379.pid
loglevel notice
logfile /var/log/redis/redis.log

# Data persistence
dir /var/lib/redis
save 900 1
save 300 10
save 60 10000

# Memory management
maxmemory-policy allkeys-lru

# Security
# requirepass yourpassword

# Modules
loadmodule /usr/lib/redis/modules/libredis_http.so

# HTTP Module Configuration (if needed)
# module-http-port 4887
# module-http-bind 0.0.0.0

EOF

# Set proper ownership of configuration file
RUN chown ${REDIS_USER}:${REDIS_GROUP} ${REDIS_CONF_DIR}/redis.conf

# Create startup script
RUN cat > /usr/local/bin/start-redis.sh << 'EOF'
#!/bin/bash
set -e

echo "Starting Redis 8.2.1 with HTTP module..."

# Verify module is present
if [ ! -f /usr/lib/redis/modules/libredis_http.so ]; then
    echo "ERROR: Redis HTTP module not found at /usr/lib/redis/modules/libredis_http.so"
    exit 1
fi

# Verify module is loadable
echo "Verifying Redis HTTP module..."
file /usr/lib/redis/modules/libredis_http.so
ldd /usr/lib/redis/modules/libredis_http.so

# Start Redis server
echo "Starting Redis server..."
exec redis-server /etc/redis/redis.conf
EOF

RUN chmod +x /usr/local/bin/start-redis.sh

# Expose ports
EXPOSE 6379 4887

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD redis-cli ping || exit 1

# Create volume for Redis data
VOLUME ["/var/lib/redis"]

# Switch to redis user
USER ${REDIS_USER}

# Set working directory
WORKDIR ${REDIS_HOME}

# Start Redis
CMD ["/usr/local/bin/start-redis.sh"]
