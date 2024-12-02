FROM postgres:latest

ENV POSTGRES_USER=trade_user
ENV POSTGRES_PASSWORD=password_trade_123
ENV POSTGRES_DB=trade_with_me

EXPOSE 5432

CMD ["postgres"]
