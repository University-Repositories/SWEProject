-- Dev Fixture Seed File: Real MusicBrainz recordings with shared credits
-- Includes two recordings with the same title ("Hold On") by different artists
-- Includes Dan Auerbach credited as both artist and producer on "Lonely Boy"

-- Credit Entities (Real MusicBrainz UUIDs)
INSERT INTO credit_entities (id, kind, name) VALUES
    ('b6807869-d355-4422-90f7-ebbe02b11be7', 'person', 'Alabama Shakes'),
    ('7f1e72e8-5b43-41bb-9461-8ffebc43f7a4', 'person', 'The Impressions'),
    ('00ceaa2a-609d-4074-9543-a65c96b7596d', 'person', 'Curtis Mayfield'),
    ('117ce422-2977-49f3-8fbf-bf7da6e6e22c', 'person', 'Brittany Howard'),
    ('7b4f53ca-49d7-4663-b851-ea690ab9504a', 'person', 'Dan Auerbach'),
    ('d15721d8-56b4-453d-b506-c91923793612', 'person', 'The Black Keys'),
    ('c9902640-1094-4d22-b258-293c66f50fec', 'person', 'Danger Mouse'),
    ('cc0b03fb-dc01-47c0-97c5-553d5247f499', 'person', 'Adele'),
    ('c2049e39-1667-4228-b0a8-f7b5876404ad', 'person', 'Greg Kurstin'),
    ('ef327244-3118-471d-aa37-56ceec5ff7c0', 'person', 'Paul Epworth'),
    ('f357f8fe-74ce-4277-bc60-d5be86958a6a', 'label', 'ATO Records'),
    ('8f3cf5df-6a31-4a6f-b27b-232f3ff3cf87', 'label', 'Nonesuch Records'),
    ('977f6b8b-8fa2-436f-876b-c744f4ff4f21', 'label', 'XL Recordings'),
    ('26ce5539-715a-4632-9cb4-9b5cfba5b7fc', 'label', 'Curtom Records'),
    ('b7501b8e-a9ce-40bf-9ce4-e0c82f9d860e', 'album', 'Boys & Girls'),
    ('5534c03b-c2e8-4a66-b25c-9d62d6ea153d', 'album', 'Sound & Color'),
    ('2dcfaee0-e549-43c3-aebe-87002db6e300', 'album', 'El Camino'),
    ('8c3b9b46-0b6d-3e6f-8cfa-5ea7fbb05406', 'album', 'Brothers'),
    ('2a8439df-1554-469b-88a4-0a37e96b3d11', 'album', '25'),
    ('4f26046e-826d-4952-b9cf-14c622a57ef2', 'album', '21'),
    ('1219b165-27a3-4876-b31c-b72e5ee6f50b', 'album', 'This Is My Country')
ON CONFLICT (id) DO NOTHING;

-- Recordings (Real MusicBrainz recording UUIDs & release UUIDs)
INSERT INTO recordings (id, title, artist_name, year, release_id) VALUES
    ('8d227b3b-8ea9-42b4-8e54-521b7ce3a241', 'Hold On', 'Alabama Shakes', 2012, 'ef047814-1e08-41e9-89b5-d2279169b183'),
    ('1d3c01bf-2dcf-4ca6-b3b3-2ea61b17e4bb', 'Hold On', 'The Impressions', 1968, '18b6e6ce-7e61-4692-94ea-630e4e75c612'),
    ('5370d061-f09b-4654-8e43-85f269a9b7ff', 'Sound & Color', 'Alabama Shakes', 2015, 'bf944293-1498-4447-b2eb-684cbfd8e578'),
    ('8cc14e0d-e25f-44e2-a083-a74421b53e8a', 'Don''t Wanna Fight', 'Alabama Shakes', 2015, 'bf944293-1498-4447-b2eb-684cbfd8e578'),
    ('e6d23485-9a64-4e78-9e6b-cf83a99e742c', 'Lonely Boy', 'The Black Keys', 2011, 'd07bb8a8-0624-4f9e-bb0a-4a69cf4f4cfd'),
    ('c7a407e3-057d-41a3-83ea-607823522f18', 'Tighten Up', 'The Black Keys', 2010, '3a7d4e5f-bf10-4e3a-967f-47dc37eb931e'),
    ('2ea37b67-bf1b-430c-8e89-13e618821a7d', 'Gold on the Ceiling', 'The Black Keys', 2011, 'd07bb8a8-0624-4f9e-bb0a-4a69cf4f4cfd'),
    ('4b533db1-0943-4318-ab93-41a4a4086695', 'Rolling in the Deep', 'Adele', 2010, '9dcbc58a-aef2-48f0-b98a-7c9c0b1ef38a'),
    ('8fbcf1e7-8898-4856-9a25-c6bf2659e55b', 'Hello', 'Adele', 2015, '4be5e4fa-8b17-48f5-a35c-c2b6e169c96a'),
    ('9bdfc836-3b6b-4396-857f-d31c4f1c11d6', 'Water Under the Bridge', 'Adele', 2015, '4be5e4fa-8b17-48f5-a35c-c2b6e169c96a'),
    ('a3e1eb41-1582-4114-8f7f-5a8ad8d8a7c1', 'People Get Ready', 'The Impressions', 1965, 'f5f0b83a-d68a-4ef1-9dfc-279c6d48259b'),
    ('2cebc19d-7f41-477d-895d-75e114ba3883', 'Stay High', 'Brittany Howard', 2019, '9e0618ff-a0c3-4f96-85c8-111ec0a1c1d8')
ON CONFLICT (id) DO NOTHING;

-- Recording Credits
-- Dan Auerbach is credited as both artist and producer on 'Lonely Boy'
INSERT INTO recording_credits (recording_id, credit_id, role) VALUES
    -- Hold On (Alabama Shakes)
    ('8d227b3b-8ea9-42b4-8e54-521b7ce3a241', 'b6807869-d355-4422-90f7-ebbe02b11be7', 'artist'),
    ('8d227b3b-8ea9-42b4-8e54-521b7ce3a241', '117ce422-2977-49f3-8fbf-bf7da6e6e22c', 'writer'),
    ('8d227b3b-8ea9-42b4-8e54-521b7ce3a241', 'f357f8fe-74ce-4277-bc60-d5be86958a6a', 'label'),
    ('8d227b3b-8ea9-42b4-8e54-521b7ce3a241', 'b7501b8e-a9ce-40bf-9ce4-e0c82f9d860e', 'album'),

    -- Hold On (The Impressions)
    ('1d3c01bf-2dcf-4ca6-b3b3-2ea61b17e4bb', '7f1e72e8-5b43-41bb-9461-8ffebc43f7a4', 'artist'),
    ('1d3c01bf-2dcf-4ca6-b3b3-2ea61b17e4bb', '00ceaa2a-609d-4074-9543-a65c96b7596d', 'writer'),
    ('1d3c01bf-2dcf-4ca6-b3b3-2ea61b17e4bb', '26ce5539-715a-4632-9cb4-9b5cfba5b7fc', 'label'),
    ('1d3c01bf-2dcf-4ca6-b3b3-2ea61b17e4bb', '1219b165-27a3-4876-b31c-b72e5ee6f50b', 'album'),

    -- Sound & Color (Alabama Shakes)
    ('5370d061-f09b-4654-8e43-85f269a9b7ff', 'b6807869-d355-4422-90f7-ebbe02b11be7', 'artist'),
    ('5370d061-f09b-4654-8e43-85f269a9b7ff', '117ce422-2977-49f3-8fbf-bf7da6e6e22c', 'writer'),
    ('5370d061-f09b-4654-8e43-85f269a9b7ff', 'f357f8fe-74ce-4277-bc60-d5be86958a6a', 'label'),
    ('5370d061-f09b-4654-8e43-85f269a9b7ff', '5534c03b-c2e8-4a66-b25c-9d62d6ea153d', 'album'),

    -- Don't Wanna Fight
    ('8cc14e0d-e25f-44e2-a083-a74421b53e8a', 'b6807869-d355-4422-90f7-ebbe02b11be7', 'artist'),
    ('8cc14e0d-e25f-44e2-a083-a74421b53e8a', '117ce422-2977-49f3-8fbf-bf7da6e6e22c', 'writer'),
    ('8cc14e0d-e25f-44e2-a083-a74421b53e8a', '5534c03b-c2e8-4a66-b25c-9d62d6ea153d', 'album'),

    -- Lonely Boy (The Black Keys) - Dan Auerbach is both artist and producer
    ('e6d23485-9a64-4e78-9e6b-cf83a99e742c', 'd15721d8-56b4-453d-b506-c91923793612', 'artist'),
    ('e6d23485-9a64-4e78-9e6b-cf83a99e742c', '7b4f53ca-49d7-4663-b851-ea690ab9504a', 'artist'),
    ('e6d23485-9a64-4e78-9e6b-cf83a99e742c', '7b4f53ca-49d7-4663-b851-ea690ab9504a', 'producer'),
    ('e6d23485-9a64-4e78-9e6b-cf83a99e742c', 'c9902640-1094-4d22-b258-293c66f50fec', 'producer'),
    ('e6d23485-9a64-4e78-9e6b-cf83a99e742c', '8f3cf5df-6a31-4a6f-b27b-232f3ff3cf87', 'label'),
    ('e6d23485-9a64-4e78-9e6b-cf83a99e742c', '2dcfaee0-e549-43c3-aebe-87002db6e300', 'album'),

    -- Tighten Up (The Black Keys)
    ('c7a407e3-057d-41a3-83ea-607823522f18', 'd15721d8-56b4-453d-b506-c91923793612', 'artist'),
    ('c7a407e3-057d-41a3-83ea-607823522f18', 'c9902640-1094-4d22-b258-293c66f50fec', 'producer'),
    ('c7a407e3-057d-41a3-83ea-607823522f18', '8c3b9b46-0b6d-3e6f-8cfa-5ea7fbb05406', 'album'),

    -- Gold on the Ceiling (The Black Keys)
    ('2ea37b67-bf1b-430c-8e89-13e618821a7d', 'd15721d8-56b4-453d-b506-c91923793612', 'artist'),
    ('2ea37b67-bf1b-430c-8e89-13e618821a7d', 'c9902640-1094-4d22-b258-293c66f50fec', 'producer'),
    ('2ea37b67-bf1b-430c-8e89-13e618821a7d', '2dcfaee0-e549-43c3-aebe-87002db6e300', 'album'),

    -- Rolling in the Deep (Adele)
    ('4b533db1-0943-4318-ab93-41a4a4086695', 'cc0b03fb-dc01-47c0-97c5-553d5247f499', 'artist'),
    ('4b533db1-0943-4318-ab93-41a4a4086695', 'ef327244-3118-471d-aa37-56ceec5ff7c0', 'producer'),
    ('4b533db1-0943-4318-ab93-41a4a4086695', '977f6b8b-8fa2-436f-876b-c744f4ff4f21', 'label'),
    ('4b533db1-0943-4318-ab93-41a4a4086695', '4f26046e-826d-4952-b9cf-14c622a57ef2', 'album'),

    -- Hello (Adele)
    ('8fbcf1e7-8898-4856-9a25-c6bf2659e55b', 'cc0b03fb-dc01-47c0-97c5-553d5247f499', 'artist'),
    ('8fbcf1e7-8898-4856-9a25-c6bf2659e55b', 'c2049e39-1667-4228-b0a8-f7b5876404ad', 'producer'),
    ('8fbcf1e7-8898-4856-9a25-c6bf2659e55b', '977f6b8b-8fa2-436f-876b-c744f4ff4f21', 'label'),
    ('8fbcf1e7-8898-4856-9a25-c6bf2659e55b', '2a8439df-1554-469b-88a4-0a37e96b3d11', 'album'),

    -- Water Under the Bridge (Adele)
    ('9bdfc836-3b6b-4396-857f-d31c4f1c11d6', 'cc0b03fb-dc01-47c0-97c5-553d5247f499', 'artist'),
    ('9bdfc836-3b6b-4396-857f-d31c4f1c11d6', 'c2049e39-1667-4228-b0a8-f7b5876404ad', 'producer'),
    ('9bdfc836-3b6b-4396-857f-d31c4f1c11d6', '2a8439df-1554-469b-88a4-0a37e96b3d11', 'album'),

    -- People Get Ready (The Impressions)
    ('a3e1eb41-1582-4114-8f7f-5a8ad8d8a7c1', '7f1e72e8-5b43-41bb-9461-8ffebc43f7a4', 'artist'),
    ('a3e1eb41-1582-4114-8f7f-5a8ad8d8a7c1', '00ceaa2a-609d-4074-9543-a65c96b7596d', 'writer'),
    ('a3e1eb41-1582-4114-8f7f-5a8ad8d8a7c1', '26ce5539-715a-4632-9cb4-9b5cfba5b7fc', 'label'),

    -- Stay High (Brittany Howard)
    ('2cebc19d-7f41-477d-895d-75e114ba3883', '117ce422-2977-49f3-8fbf-bf7da6e6e22c', 'artist'),
    ('2cebc19d-7f41-477d-895d-75e114ba3883', 'f357f8fe-74ce-4277-bc60-d5be86958a6a', 'label')
ON CONFLICT (recording_id, credit_id, role) DO NOTHING;
