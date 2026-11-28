N10 (Summary, written by PutiaCNC)
N20 (Spindle (!):            Not run by the program: it stops -M0- to ask you to start it at the)
N30 (                        speed shown, and again before a tool change and at the end to stop it.)
N40 (                        There is no work coordinate system: jog to the zero of the job and set)
N50 (                        it with G92 X0 Y0 Z0 first.)
N60 (Tools:                  3 mm flat end mill, 6 mm flat end mill. No tool change.)
N70 (Pauses:                 The program stops 4 times with the spindle off: 'Start the spindle at)
N80 (                        12000 rpm, then resume', 'Stop the spindle and change tool to 6 mm,)
N90 (                        then touch off Z 0 and resume', 'Start the spindle at 10000 rpm, then)
N100 (                        resume', 'Stop the spindle, then resume'. Resume it when you are)
N110 (                        ready; the spindle starts again by itself.)
N120 (Cutting:                Down to Z -1 mm. 94 mm of cutting, about 11 s in all.)
N130 (End:                    Lifts to Z 5 mm, goes to X0 Y0.)
N140 ; PutiaCNC - Marlin (no spindle control)
N150 ; Stock 60 x 40 x 6 mm; XY0 = lower-left corner of stock, Z0 = top surface
N160 G21 G90
N170 ; Marlin: jog the tool to the zero of the job and set it there with G92 X0 Y0 Z0 before you run this file
N180 ; Spindle: by hand. The program stops to ask you to start and stop it
N190 G0 Z5
N200 ; Tool: 3 mm flat end mill
N210 M0 Start the spindle at 12000 rpm, then resume
N220 ; Operation 1: outside, tool 3 mm
N230 G0 X20.344 Y3.54
N240 G0 Z0.5
N250 G1 Z0 F250
N260 ; Pass 1
N270 G1 Z-1
N280 G1 F800
N290 G1 X20.669 Y3.657
N300 G1 X20.959 Y3.846
N310 G1 X21.197 Y4.096
N320 G1 X21.372 Y4.395
N330 G1 X21.475 Y4.725
N340 G1 X21.5 Y5
N350 G1 Y15
N360 G1 X21.46 Y15.344
N370 G1 X21.343 Y15.669
N380 G1 X21.154 Y15.959
N390 G1 X20.904 Y16.197
N400 G1 X20.605 Y16.372
N410 G1 X20.275 Y16.475
N420 G1 X20 Y16.5
N430 G1 X5
N440 G1 X4.656 Y16.46
N450 G1 X4.331 Y16.343
N460 G1 X4.041 Y16.154
N470 G1 X3.803 Y15.904
N480 G1 X3.628 Y15.605
N490 G1 X3.525 Y15.275
N500 G1 X3.5 Y15
N510 G1 Y5
N520 G1 X3.54 Y4.656
N530 G1 X3.657 Y4.331
N540 G1 X3.846 Y4.041
N550 G1 X4.096 Y3.803
N560 G1 X4.395 Y3.628
N570 G1 X4.725 Y3.525
N580 G1 X5 Y3.5
N590 G1 X20
N600 G1 X20.344 Y3.54
N610 G0 Z5
N620 M0 Stop the spindle and change tool to 6 mm, then touch off Z 0 and resume
N630 ; Tool: 6 mm flat end mill
N640 M0 Start the spindle at 10000 rpm, then resume
N650 ; Operation 2: inside, tool 6 mm
N660 G0 X44.618 Y20.015
N670 G0 Z0.5
N680 G1 Z0 F300
N690 ; Pass 1
N700 G1 Z-1
N710 G1 F1000
N720 G1 X44.122 Y20.078
N730 G1 X43.645 Y20.187
N740 G1 X43.153 Y20.354
N750 G1 X42.714 Y20.553
N760 G1 X42.282 Y20.803
N770 G1 X41.877 Y21.095
N780 G1 X41.492 Y21.437
N790 G1 X41.146 Y21.815
N800 G1 X40.875 Y22.174
N810 G1 X40.604 Y22.617
N820 G1 X40.377 Y23.095
N830 G1 X40.222 Y23.528
N840 G1 X40.101 Y24.005
N850 G1 X40.001 Y24.884
N860 G1 X40.015 Y25.382
N870 G1 X40.078 Y25.878
N880 G1 X40.187 Y26.355
N890 G1 X40.354 Y26.847
N900 G1 X40.553 Y27.286
N910 G1 X40.803 Y27.718
N920 G1 X41.095 Y28.123
N930 G1 X41.437 Y28.508
N940 G1 X41.782 Y28.826
N950 G1 X42.174 Y29.125
N960 G1 X42.617 Y29.396
N970 G1 X43.079 Y29.616
N980 G1 X43.528 Y29.778
N990 G1 X44.022 Y29.903
N1000 G1 X44.884 Y29.999
N1010 G1 X45.382 Y29.985
N1020 G1 X45.878 Y29.922
N1030 G1 X46.355 Y29.813
N1040 G1 X46.847 Y29.646
N1050 G1 X47.286 Y29.447
N1060 G1 X47.718 Y29.197
N1070 G1 X48.123 Y28.905
N1080 G1 X48.508 Y28.563
N1090 G1 X48.854 Y28.185
N1100 G1 X49.125 Y27.826
N1110 G1 X49.396 Y27.383
N1120 G1 X49.623 Y26.905
N1130 G1 X49.778 Y26.472
N1140 G1 X49.903 Y25.978
N1150 G1 X49.999 Y25.116
N1160 G1 X49.984 Y24.601
N1170 G1 X49.919 Y24.107
N1180 G1 X49.813 Y23.645
N1190 G1 X49.642 Y23.142
N1200 G1 X49.447 Y22.714
N1210 G1 X49.185 Y22.264
N1220 G1 X48.905 Y21.877
N1230 G1 X48.563 Y21.492
N1240 G1 X48.196 Y21.156
N1250 G1 X47.826 Y20.875
N1260 G1 X47.383 Y20.604
N1270 G1 X46.921 Y20.384
N1280 G1 X46.472 Y20.222
N1290 G1 X45.978 Y20.097
N1300 G1 X45.116 Y20.001
N1310 G1 X44.618 Y20.015
N1320 G0 Z5
N1330 M0 Stop the spindle, then resume
N1340 G0 X0 Y0
N1350 ; End of program