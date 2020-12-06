--
-- PostgreSQL database dump
--

-- Dumped from database version 12.4
-- Dumped by pg_dump version 12.4

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: geodata; Type: TABLE; Schema: geohub
--

CREATE SCHEMA IF NOT EXISTS geohub;

CREATE TABLE geohub.geodata (
    id serial primary key,
    client text not null,
    lat double precision,
    long double precision,
    spd double precision,
    accuracy double precision,
    t timestamp with time zone not null,
    ele double precision
    secret bytea,
    note text
);


--
-- PostgreSQL database dump complete
--

