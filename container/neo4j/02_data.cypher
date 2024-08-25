:USE TASKSCORE;

MATCH (n) DETACH DELETE n;

CREATE (u_fl:User {username: 'roterkohl', display_name: 'Flori', pwd_hash: '$2b$12$bpojR9PuVfpoRs7nqONwGuf/kXD9tIKgxQyCT1CoZxz6KYYDb4mPG', is_admin: true, points: 187 }), // Flori1234
	(u_mi:User {username: 'brutours.de', display_name: 'Michi', pwd_hash: '	$2b$12$gIcYE/nRDJ3E2lLWSRTw1.0PhDIVlyivKfbogz2XYTiu4k2gkrt96', points: 167 }),	// Michi1234
	(u_fr:User {username: 'dliwespf', display_name: 'Franki', pwd_hash: '$2b$12$4cTgPxlCKKngPqhHo5MJwOAoZxaRVZGSbioUpycjJTCB1Z/DLJ/BW', points: 0 }),	// Franki1234
	(u_to:User {username: 'topher', display_name: 'Topher', pwd_hash: '	$2b$12$mNX9uQs44Jv08fzVfCWXmO7P6rPE4bMnqgCSZ16RLZNAqho/OqGdC', is_admin: true, points: 475 }),	//Topheri1234

	(t_bl:Task { name: 'Blumen gießen', points: 10, enabled: true }),
	(t_st:Task { name: 'Stunden abgeben', points: 30, enabled: false }),
	(t_sp:Task { name: 'Spülmaschine ausräumen', points: 52, enabled: true }),
	(t_ka:Task { name: 'Kaffee kochen', points: 75, enabled: true }),
	
	(te_ba: Team { id: 1, name: 'Babes' }),
	(te_ch: Team { id: 2, name: 'Church' }),
	(te_ba)-[:MANAGED_BY] -> (u_fl),
	(te_ch)-[:MANAGED_BY] -> (u_mi),
	
	(te_ba)-[:MEMBER] -> (u_fl),
	(te_ch)-[:MEMBER] -> (u_mi),
	(te_ch)-[:MEMBER] -> (u_fr),
	(te_ch)-[:MEMBER] -> (u_fl),
	(te_ch)-[:MEMBER] -> (u_to),
	
	(u_fl)-[:SCORED {points: 10, scored_at: localdatetime()}] -> (t_bl),
	(u_fl)-[:SCORED {points: 10, scored_at: localdatetime()}] -> (t_bl),
	(u_fl)-[:SCORED {points: 30, scored_at: localdatetime()}] -> (t_st),
	(u_fl)-[:SCORED {points: 10, scored_at: localdatetime()}] -> (t_bl),
	(u_fl)-[:SCORED {points: 75, scored_at: localdatetime()}] -> (t_ka),
	(u_fl)-[:SCORED {points: 52, scored_at: localdatetime()}] -> (t_sp),
	
	(u_mi)-[:SCORED {points: 10, scored_at: localdatetime()}] -> (t_bl),
	(u_mi)-[:SCORED {points: 30, scored_at: localdatetime()}] -> (t_st),
	(u_mi)-[:SCORED {points: 52, scored_at: localdatetime()}] -> (t_sp),
	(u_mi)-[:SCORED {points: 75, scored_at: localdatetime()}] -> (t_ka),
	
	(u_to)-[:SCORED {points: 75, scored_at: localdatetime()}] -> (t_ka),
	(u_to)-[:SCORED {points: 75, scored_at: localdatetime()}] -> (t_ka),
	(u_to)-[:SCORED {points: 75, scored_at: localdatetime()}] -> (t_ka),
	(u_to)-[:SCORED {points: 75, scored_at: localdatetime()}] -> (t_ka),
	(u_to)-[:SCORED {points: 75, scored_at: localdatetime()}] -> (t_ka)
	
