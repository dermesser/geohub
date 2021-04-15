--
-- PostgreSQL database dump
--

-- Dumped from database version 12.6
-- Dumped by pg_dump version 12.6

-- pg_dump -s  -U <owner> -n geohub -O

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

--
-- Name: geohub; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA geohub;


SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: geodata; Type: TABLE; Schema: geohub; Owner: -
--

CREATE TABLE geohub.geodata (
    client text NOT NULL,
    lat double precision,
    long double precision,
    spd double precision,
    t timestamp with time zone,
    ele double precision,
    secret bytea,
    id integer NOT NULL,
    accuracy double precision,
    note text
);


--
-- Name: geodata_id_seq; Type: SEQUENCE; Schema: geohub; Owner: -
--

CREATE SEQUENCE geohub.geodata_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: geodata_id_seq; Type: SEQUENCE OWNED BY; Schema: geohub; Owner: -
--

ALTER SEQUENCE geohub.geodata_id_seq OWNED BY geohub.geodata.id;


--
-- Name: geodata id; Type: DEFAULT; Schema: geohub; Owner: -
--

ALTER TABLE ONLY geohub.geodata ALTER COLUMN id SET DEFAULT nextval('geohub.geodata_id_seq'::regclass);


--
-- Name: geodata geodata_pkey; Type: CONSTRAINT; Schema: geohub; Owner: -
--

ALTER TABLE ONLY geohub.geodata
    ADD CONSTRAINT geodata_pkey PRIMARY KEY (id);


--
-- Name: geodata_client_secret_idx; Type: INDEX; Schema: geohub; Owner: -
--

CREATE INDEX geodata_client_secret_idx ON geohub.geodata USING btree (client, secret);


--
-- Name: geodata_t_idx; Type: INDEX; Schema: geohub; Owner: -
--

CREATE INDEX geodata_t_idx ON geohub.geodata USING btree (t);


--
-- PostgreSQL database dump complete
--

