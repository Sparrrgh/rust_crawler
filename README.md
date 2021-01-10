I created this tool solely as an exercise to familiarize myself with parallel programming in Rust.  
It simply **scans** networks searching for webservers and, when it finds one, it **screenshots** the landing page.

The final objective is to scan a /16 subnet in less than 15 minutes.

It has two modes, one uses file containing the endpoints to test (one for each line). The other mode tests an address block given the start and the end of the block. 
**Watch out!** the endpoints tested using the IP address block comprehend both the starting and the trailing IP.  
```
Usage:  ./crawler endpoints_file output_directory
	./crawler start_address_block end_address_block output_directory
```
Example:  
`./crawler 192.168.0.1 192.168.254.254 my_local_network`
<br>
### Why is it called "crawler" if it doesn't actually crawl webpages?
Because I'm bad at naming stuff, maybe I'll implement an actual crawler in the future but as of right now it's outside of the scope of the project.  
<br>
I would also like to add the possibility to test non-standard ports, but I'm too busy with exams atm.
