import glob
import os.path
import time
from outdated_script import const
from selenium import webdriver
from selenium.webdriver.support import expected_conditions, ui
import cv2
from PIL import Image
import numpy as np

print(webdriver.__version__)

options = webdriver.ChromeOptions()
options.headless = True
options.add_argument('--hide-scrollbars')

driver = webdriver.Chrome(options=options)

for name in const.playlists():
    if not name[2]:
        continue
    print(name[1])
    html_path = os.path.join(os.getcwd(), 'html', name[1] + '.html')
    print(html_path, end='\n\n')
    driver.get("file://" + html_path)
    ui.WebDriverWait(driver=driver, timeout=15).until(expected_conditions.presence_of_all_elements_located)
    time.sleep(3)

    driver.set_window_size(800, 600)
    driver.set_window_size(driver.execute_script('return document.body.scrollWidth') + 40,
                           driver.execute_script('return document.body.scrollHeight') + 40)

    with open(os.path.join(os.getcwd(), 'images', name[1] + '_2.png'), mode='wb') as f:
        f.write(driver.get_screenshot_as_png())

for filename in glob.glob(os.path.join('images', '*_2.png')):
    print(filename)

    image = cv2.cvtColor(np.array(Image.open(os.path.join(os.getcwd(), filename))), cv2.COLOR_RGB2BGR)
    splits = image.shape[0] // 4000 + 1
    print(splits)
    cx = cy = 0
    h, w, _ = image.shape
    if splits == 1:
        continue

    for i in range(splits):
        split_pic = image[cy:cy + int(h / splits), cx:cx + w, :]
        Image.fromarray(cv2.cvtColor(split_pic, cv2.COLOR_BGR2RGB)).save(filename.replace('_2', '_2' + str(i)))
        cy = cy + int(h / splits)
    cy = 0
    os.remove(filename)
